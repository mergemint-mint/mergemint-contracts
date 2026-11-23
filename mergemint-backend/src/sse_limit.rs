//! Caps on concurrent `GET /bounties/stream` connections (#1118).
//!
//! An SSE connection stays open for as long as the client wants, so one
//! client could open thousands and use up the server's sockets and memory.
//! [`SseConnectionLimiter`] enforces two caps:
//!
//! * a per-IP cap (`SSE_MAX_CONNECTIONS_PER_IP`, default 32), and
//! * a global cap across all clients (`SSE_MAX_CONNECTIONS`, default 1024).
//!
//! Each accepted stream holds an [`SseConnectionPermit`]. The permit is
//! dropped with the stream, so the slot is freed when the client disconnects.
//! When a cap is reached the handler returns `429 Too Many Requests`.
//!
//! Behind a reverse proxy every client has the proxy's IP. Set
//! `SSE_TRUST_X_FORWARDED_FOR=true` to key the per-IP cap on the last address
//! in `X-Forwarded-For` (the one the nearest proxy added). Only enable it when
//! a proxy you control sets that header, or clients can spoof it.
//!
//! When `SSE_TRUSTED_PROXIES` is set to a comma-separated CIDR list, only
//! peers whose address falls within one of those ranges may set
//! `X-Forwarded-For`.  IPv6 peers are bucketed by their `/64` prefix so an
//! attacker with a `/64` allocation cannot bypass `SSE_MAX_CONNECTIONS_PER_IP`
//! by rotating interface IDs.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::http::HeaderMap;

pub const MAX_PER_IP_ENV: &str = "SSE_MAX_CONNECTIONS_PER_IP";
pub const MAX_GLOBAL_ENV: &str = "SSE_MAX_CONNECTIONS";
pub const TRUST_FORWARDED_FOR_ENV: &str = "SSE_TRUST_X_FORWARDED_FOR";
pub const TRUSTED_PROXIES_ENV: &str = "SSE_TRUSTED_PROXIES";

pub const DEFAULT_MAX_PER_IP: usize = 32;
pub const DEFAULT_MAX_GLOBAL: usize = 1024;

/// Which cap refused a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitExceeded {
    PerIp,
    Global,
}

impl LimitExceeded {
    pub fn message(self) -> &'static str {
        match self {
            Self::PerIp => "too many concurrent stream connections from this address",
            Self::Global => "server stream connection limit reached, retry later",
        }
    }
}

#[derive(Debug, Default)]
struct Counts {
    total: usize,
    per_ip: HashMap<IpAddr, usize>,
}

/// A CIDR range: network address plus prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cidr {
    addr: IpAddr,
    prefix: u8,
}

impl Cidr {
    /// Returns `true` when `ip` falls within this CIDR range.
    fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(host)) => {
                if self.prefix > 32 {
                    return false;
                }
                let mask: u32 = if self.prefix == 0 { 0 } else { !0u32 << (32 - self.prefix) };
                u32::from(net) & mask == u32::from(host) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(host)) => {
                if self.prefix > 128 {
                    return false;
                }
                let net_segs = net.segments();
                let host_segs = host.segments();
                let mut bits = self.prefix;
                for i in 0..8 {
                    if bits == 0 {
                        return true;
                    }
                    if bits >= 16 {
                        if net_segs[i] != host_segs[i] {
                            return false;
                        }
                        bits -= 16;
                        continue;
                    }
                    let seg_mask: u16 = !0u16 << (16 - bits);
                    if (net_segs[i] & seg_mask) != (host_segs[i] & seg_mask) {
                        return false;
                    }
                    return true;
                }
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
struct Inner {
    max_per_ip: usize,
    max_global: usize,
    trust_forwarded_for: bool,
    trusted_proxies: Vec<Cidr>,
    counts: Mutex<Counts>,
    rejected_per_ip: AtomicU64,
    rejected_global: AtomicU64,
}

/// Tracks open SSE connections. Cheap to clone; clones share counts.
#[derive(Debug, Clone)]
pub struct SseConnectionLimiter {
    inner: Arc<Inner>,
}

impl Default for SseConnectionLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PER_IP, DEFAULT_MAX_GLOBAL)
    }
}

impl SseConnectionLimiter {
    /// A limiter with the given caps. A cap of `0` means no limit.
    pub fn new(max_per_ip: usize, max_global: usize) -> Self {
        Self::with_options(max_per_ip, max_global, false)
    }

    /// Like [`new`](Self::new), and optionally keys the per-IP cap on
    /// `X-Forwarded-For`.
    pub fn with_options(max_per_ip: usize, max_global: usize, trust_forwarded_for: bool) -> Self {
        Self {
            inner: Arc::new(Inner {
                max_per_ip,
                max_global,
                trust_forwarded_for,
                trusted_proxies: Vec::new(),
                counts: Mutex::new(Counts::default()),
                rejected_per_ip: AtomicU64::new(0),
                rejected_global: AtomicU64::new(0),
            }),
        }
    }

    /// Restrict `X-Forwarded-For` honouring to peers whose address falls within
    /// one of these CIDR ranges.  When the list is empty the header is never
    /// trusted even if `SSE_TRUST_X_FORWARDED_FOR=true`.
    pub fn with_trusted_proxies(mut self, proxies: Vec<Cidr>) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.trusted_proxies = proxies;
        }
        self
    }

    /// Read caps from `SSE_MAX_CONNECTIONS_PER_IP`, `SSE_MAX_CONNECTIONS` and
    /// `SSE_TRUST_X_FORWARDED_FOR`. Bad values fall back to the defaults.
    pub fn from_env() -> Self {
        let per_ip = read_cap(MAX_PER_IP_ENV, DEFAULT_MAX_PER_IP);
        let global = read_cap(MAX_GLOBAL_ENV, DEFAULT_MAX_GLOBAL);
        let trust = std::env::var(TRUST_FORWARDED_FOR_ENV)
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        let proxies = read_trusted_proxies(TRUSTED_PROXIES_ENV);
        tracing::info!(
            max_per_ip = per_ip,
            max_global = global,
            trust_forwarded_for = trust,
            trusted_proxies = ?proxies,
            "SSE connection limits configured (0 = unlimited)"
        );
        Self::with_options(per_ip, global, trust).with_trusted_proxies(proxies)
    }

    /// The address used as the per-IP key for a request.
    pub fn client_ip(&self, headers: &HeaderMap, peer: Option<SocketAddr>) -> Option<IpAddr> {
        let peer_ip = peer.map(|addr| canonical_ip(addr.ip()));

        // Only honour `X-Forwarded-For` when the peer is a trusted proxy.
        // Otherwise clients can spoof the header to bypass the per-IP cap.
        if self.inner.trust_forwarded_for {
            if let Some(peer) = peer_ip {
                if self
                    .inner
                    .trusted_proxies
                    .iter()
                    .any(|net| net.contains(peer))
                {
                    if let Some(forwarded) = parse_forwarded_for(headers) {
                        return Some(forwarded);
                    }
                }
            }
            return peer_ip;
        }
        peer_ip
    }

    /// Take a slot for a new connection from `ip`. When `ip` is unknown only
    /// the global cap applies.
    pub fn try_acquire(&self, ip: Option<IpAddr>) -> Result<SseConnectionPermit, LimitExceeded> {
        let mut counts = self.inner.counts.lock().unwrap_or_else(|e| e.into_inner());

        if self.inner.max_global != 0 && counts.total >= self.inner.max_global {
            self.inner.rejected_global.fetch_add(1, Ordering::Relaxed);
            return Err(LimitExceeded::Global);
        }
        if let Some(ip) = ip {
            let key = subnet_key(ip);
            let current = counts.per_ip.get(&key).copied().unwrap_or(0);
            if self.inner.max_per_ip != 0 && current >= self.inner.max_per_ip {
                self.inner.rejected_per_ip.fetch_add(1, Ordering::Relaxed);
                return Err(LimitExceeded::PerIp);
            }
            counts.per_ip.insert(key, current + 1);
        }
        counts.total += 1;

        Ok(SseConnectionPermit {
            inner: self.inner.clone(),
            ip,
        })
    }

    /// Open connections right now.
    pub fn active(&self) -> usize {
        self.inner
            .counts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total
    }

    /// Open connections from one address.
    #[cfg(test)]
    pub fn active_for(&self, ip: IpAddr) -> usize {
        self.inner
            .counts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .per_ip
            .get(&subnet_key(ip))
            .copied()
            .unwrap_or(0)
    }

    /// Connections refused by the given cap since startup.
    pub fn rejected(&self, reason: LimitExceeded) -> u64 {
        match reason {
            LimitExceeded::PerIp => self.inner.rejected_per_ip.load(Ordering::Relaxed),
            LimitExceeded::Global => self.inner.rejected_global.load(Ordering::Relaxed),
        }
    }
}

/// A held connection slot. Dropping it frees the slot.
#[derive(Debug)]
pub struct SseConnectionPermit {
    inner: Arc<Inner>,
    ip: Option<IpAddr>,
}

impl Drop for SseConnectionPermit {
    fn drop(&mut self) {
        let mut counts = self.inner.counts.lock().unwrap_or_else(|e| e.into_inner());
        counts.total = counts.total.saturating_sub(1);
        if let Some(ip) = self.ip {
            let key = subnet_key(ip);
            if let Some(n) = counts.per_ip.get_mut(&key) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    counts.per_ip.remove(&key);
                }
            }
        }
    }
}

/// Treat `::ffff:a.b.c.d` as `a.b.c.d` so a dual-stack listener does not
/// give one client two separate buckets.
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        IpAddr::V4(v4) => IpAddr::V4(v4),
    }
}

/// Normalize an IPv6 address to its `/64` prefix so that all hosts in the same
/// `/64` subnet share a single per-IP bucket.  IPv4 addresses are unchanged.
fn subnet_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            let segs = v6.segments();
            IpAddr::V6(Ipv6Addr::new(segs[0], segs[1], segs[2], segs[3], 0, 0, 0, 0))
        }
        IpAddr::V4(v4) => IpAddr::V4(v4),
    }
}

/// Extract the last valid IP from `X-Forwarded-For`, or `None` if the header
/// is missing or contains no parseable address.
fn parse_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|part| part.trim().parse::<IpAddr>().ok())
        .next_back()
}

/// Parse a comma-separated list of CIDR ranges (e.g. `"10.0.0.0/8,2001:db8::/32"`)
/// into `Cidr` entries.  Unparseable entries are skipped with a warning.
fn read_trusted_proxies(name: &str) -> Vec<Cidr> {
    match std::env::var(name) {
        Ok(raw) => raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|cidr| {
                let (addr, prefix) = cidr.split_once('/')?;
                let addr: IpAddr = addr.trim().parse().ok()?;
                let prefix: u8 = prefix.trim().parse().ok()?;
                Some(Cidr { addr, prefix })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn read_cap(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(raw) => raw.trim().parse::<usize>().unwrap_or_else(|_| {
            tracing::warn!(
                env = name,
                value = raw,
                default,
                "invalid SSE connection cap; using default"
            );
            default
        }),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn per_ip_cap_is_enforced_and_released_on_drop() {
        let limiter = SseConnectionLimiter::new(2, 100);
        let a = ip("203.0.113.1");

        let first = limiter.try_acquire(Some(a)).unwrap();
        let second = limiter.try_acquire(Some(a)).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(a)).unwrap_err(),
            LimitExceeded::PerIp
        );
        assert_eq!(limiter.active_for(a), 2);
        assert_eq!(limiter.rejected(LimitExceeded::PerIp), 1);

        // Another address is unaffected.
        let other = limiter.try_acquire(Some(ip("203.0.113.2"))).unwrap();
        assert_eq!(limiter.active(), 3);

        drop(first);
        assert_eq!(limiter.active_for(a), 1);
        let third = limiter.try_acquire(Some(a)).unwrap();

        drop((second, third, other));
        assert_eq!(limiter.active(), 0);
        assert_eq!(limiter.active_for(a), 0);
    }

    #[test]
    fn global_cap_is_enforced_across_addresses() {
        let limiter = SseConnectionLimiter::new(10, 2);
        let _a = limiter.try_acquire(Some(ip("198.51.100.1"))).unwrap();
        let b = limiter.try_acquire(Some(ip("198.51.100.2"))).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(ip("198.51.100.3"))).unwrap_err(),
            LimitExceeded::Global
        );
        // Unknown peers still count against the global cap.
        assert_eq!(
            limiter.try_acquire(None).unwrap_err(),
            LimitExceeded::Global
        );
        assert_eq!(limiter.rejected(LimitExceeded::Global), 2);

        drop(b);
        assert!(limiter.try_acquire(None).is_ok());
    }

    #[test]
    fn zero_means_unlimited() {
        let limiter = SseConnectionLimiter::new(0, 0);
        let permits: Vec<_> = (0..500)
            .map(|_| limiter.try_acquire(Some(ip("192.0.2.9"))).unwrap())
            .collect();
        assert_eq!(limiter.active(), 500);
        drop(permits);
        assert_eq!(limiter.active(), 0);
    }

    #[test]
    fn client_ip_uses_peer_address_unless_forwarded_for_is_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 10.0.0.9".parse().unwrap());
        let peer: SocketAddr = "[::ffff:192.0.2.50]:4000".parse().unwrap();

        let untrusted = SseConnectionLimiter::new(1, 1);
        assert_eq!(
            untrusted.client_ip(&headers, Some(peer)),
            Some(ip("192.0.2.50"))
        );
        assert_eq!(untrusted.client_ip(&headers, None), None);

        let trusted = SseConnectionLimiter::with_options(1, 1, true)
            .with_trusted_proxies(vec![Cidr {
                addr: ip("192.0.2.0"),
                prefix: 24,
            }]);
        // The last hop is the one the nearest proxy appended.
        assert_eq!(
            trusted.client_ip(&headers, Some(peer)),
            Some(ip("10.0.0.9"))
        );
        // Falls back to the peer when the header is missing or garbage.
        let mut junk = HeaderMap::new();
        junk.insert("x-forwarded-for", "not-an-ip".parse().unwrap());
        assert_eq!(trusted.client_ip(&junk, Some(peer)), Some(ip("192.0.2.50")));
    }

    #[test]
    fn ipv6_peers_share_slash64_bucket() {
        let limiter = SseConnectionLimiter::new(1, 100);
        let a = ip("2001:db8:abcd:0012::1");
        let b = ip("2001:db8:abcd:0012::2"); // same /64
        let c = ip("2001:db8:abcd:0013::1"); // different /64

        let _pa = limiter.try_acquire(Some(a)).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(b)).unwrap_err(),
            LimitExceeded::PerIp
        );
        assert_eq!(limiter.try_acquire(Some(c)).is_ok(), true);
    }

    #[test]
    fn untrusted_forwarded_for_is_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.9".parse().unwrap());
        let peer: SocketAddr = "192.0.2.50:4000".parse().unwrap();

        // trust_forwarded_for=true but peer is not in trusted_proxies -> header ignored
        let trusted = SseConnectionLimiter::with_options(1, 1, true);
        assert_eq!(
            trusted.client_ip(&headers, Some(peer)),
            Some(ip("192.0.2.50"))
        );
    }
}
