//! SSRF protection for webhook target URLs (#1116).
//!
//! A webhook subscription makes the backend send POST requests to a URL the
//! caller picked. Without a check, a caller can point that URL at internal
//! services: the cloud metadata endpoint, a database admin port on the private
//! network, or the backend itself on loopback.
//!
//! Targets are checked twice:
//!
//! * **At registration**: the URL is parsed, its host resolved, and the
//!   subscription is rejected if any resolved address is blocked.
//! * **At delivery**: the webhook HTTP client uses [`GuardedResolver`], so
//!   every connection resolves the host again and refuses blocked addresses.
//!   The client connects to the exact addresses that passed the check, so a
//!   DNS record that changes between registration and delivery (DNS
//!   rebinding) cannot reach a blocked address. Hosts written as IP literals
//!   skip DNS, so [`WebhookTargetGuard::check_literal`] covers them.
//!
//! The webhook client also follows no redirects and ignores proxy settings,
//! because either one would let the request reach a host this guard never saw.
//!
//! For local development (sending webhooks to a receiver on `localhost`), set
//! `WEBHOOK_ALLOW_PRIVATE_TARGETS=true`. Never set it in production.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// Env var that turns off the private-address block. Dev only.
pub const ALLOW_PRIVATE_TARGETS_ENV: &str = "WEBHOOK_ALLOW_PRIVATE_TARGETS";

/// Which webhook targets are allowed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SsrfPolicy {
    /// When `true`, private, loopback and link-local targets are allowed.
    pub allow_private_targets: bool,
}

impl SsrfPolicy {
    /// Block every non-public address. This is the default.
    pub const fn strict() -> Self {
        Self {
            allow_private_targets: false,
        }
    }

    /// Allow private targets. For local development and tests only.
    pub const fn allow_private() -> Self {
        Self {
            allow_private_targets: true,
        }
    }

    /// Read the policy from `WEBHOOK_ALLOW_PRIVATE_TARGETS`.
    pub fn from_env() -> Self {
        Self::from_flag(std::env::var(ALLOW_PRIVATE_TARGETS_ENV).ok().as_deref())
    }

    /// Parse the override flag. Only `1`, `true` and `yes` (any case) enable it.
    pub fn from_flag(value: Option<&str>) -> Self {
        let enabled = value
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        if enabled {
            Self::allow_private()
        } else {
            Self::strict()
        }
    }

    /// Whether a connection to `ip` is allowed under this policy.
    pub fn permits(&self, ip: IpAddr) -> bool {
        self.allow_private_targets || !is_blocked_ip(ip)
    }
}

/// Returns `true` for addresses a webhook must never reach: loopback, RFC 1918
/// private ranges, link-local (which includes the 169.254.169.254 metadata
/// endpoint), carrier-grade NAT, multicast, and reserved or documentation
/// ranges. IPv6 addresses that embed an IPv4 address (mapped, compatible,
/// NAT64, 6to4) are judged by the embedded address.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => is_blocked_ipv6(v6),
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 0 // 0.0.0.0/8 "this network", includes 0.0.0.0
        || ip.is_loopback() // 127.0.0.0/8
        || ip.is_private() // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local() // 169.254/16, includes cloud metadata 169.254.169.254
        || (a == 100 && (b & 0xC0) == 64) // 100.64/10 CGNAT, includes Alibaba metadata 100.100.100.200
        || (a == 192 && b == 0 && c == 0) // 192.0.0/24 IETF protocol assignments
        || (a == 198 && (b & 0xFE) == 18) // 198.18/15 benchmarking
        || ip.is_documentation() // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || ip.is_multicast() // 224/4
        || a >= 240 // 240/4 reserved, includes broadcast 255.255.255.255
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = embedded_ipv4(ip) {
        return is_blocked_ipv4(v4);
    }
    let s = ip.segments();
    ip.is_unspecified() // ::
        || ip.is_loopback() // ::1
        || ip.is_multicast() // ff00::/8
        || (s[0] & 0xFE00) == 0xFC00 // fc00::/7 unique local, includes AWS metadata fd00:ec2::254
        || (s[0] & 0xFFC0) == 0xFE80 // fe80::/10 link-local
        || (s[0] & 0xFFC0) == 0xFEC0 // fec0::/10 deprecated site-local
        || (s[0] == 0x2001 && s[1] == 0x0DB8) // 2001:db8::/32 documentation
        || (s[0] == 0x2001 && s[1] == 0) // 2001::/32 Teredo, hides an IPv4 address
        || (s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0) // 100::/64 discard-only
}

/// The IPv4 address carried inside an IPv6 address, if it has one.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return Some(v4); // ::ffff:a.b.c.d
    }
    let s = ip.segments();
    let tail = || Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8);
    if s[..6] == [0, 0, 0, 0, 0, 0] && !ip.is_unspecified() && !ip.is_loopback() {
        return Some(tail()); // ::a.b.c.d (deprecated IPv4-compatible)
    }
    if s[..6] == [0x64, 0xFF9B, 0, 0, 0, 0] {
        return Some(tail()); // 64:ff9b::/96 NAT64
    }
    if s[0] == 0x2002 {
        // 2002::/16 6to4: the IPv4 address sits in segments 1 and 2.
        return Some(Ipv4Addr::new(
            (s[1] >> 8) as u8,
            s[1] as u8,
            (s[2] >> 8) as u8,
            s[2] as u8,
        ));
    }
    None
}

/// Why a webhook target was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    InvalidUrl(String),
    UnsupportedScheme(String),
    MissingHost,
    BlockedAddress { host: String, ip: IpAddr },
    Unresolvable { host: String, reason: String },
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(reason) => write!(f, "invalid webhook URL: {reason}"),
            Self::UnsupportedScheme(scheme) => write!(
                f,
                "invalid webhook URL: scheme '{scheme}' is not allowed, use http or https"
            ),
            Self::MissingHost => write!(f, "invalid webhook URL: missing host"),
            Self::BlockedAddress { host, ip } if host.parse::<IpAddr>().ok() == Some(*ip) => {
                write!(
                    f,
                    "webhook target {ip} is a private, loopback, link-local or reserved address"
                )
            }
            Self::BlockedAddress { host, ip } => write!(
                f,
                "webhook target '{host}' resolves to {ip}, which is a private, loopback, \
                 link-local or reserved address"
            ),
            Self::Unresolvable { host, reason } => {
                write!(f, "webhook target '{host}' could not be resolved: {reason}")
            }
        }
    }
}

impl std::error::Error for TargetError {}

/// Future returned by [`HostResolver::lookup`].
pub type LookupFuture = Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>>;

/// DNS lookup used by the guard. Tests swap in a fake one to simulate DNS
/// rebinding.
pub trait HostResolver: Send + Sync + 'static {
    fn lookup(&self, host: &str) -> LookupFuture;
}

/// Resolves hosts with the operating system resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemResolver;

impl HostResolver for SystemResolver {
    fn lookup(&self, host: &str) -> LookupFuture {
        let host = host.to_string();
        Box::pin(async move {
            let addrs = tokio::net::lookup_host((host.as_str(), 0)).await?;
            Ok(addrs.map(|addr| addr.ip()).collect())
        })
    }
}

/// Checks webhook targets against an [`SsrfPolicy`].
#[derive(Clone)]
pub struct WebhookTargetGuard {
    policy: SsrfPolicy,
    resolver: Arc<dyn HostResolver>,
}

impl Default for WebhookTargetGuard {
    fn default() -> Self {
        Self::new(SsrfPolicy::strict())
    }
}

impl std::fmt::Debug for WebhookTargetGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookTargetGuard")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl WebhookTargetGuard {
    /// A guard that uses the system DNS resolver.
    pub fn new(policy: SsrfPolicy) -> Self {
        Self::with_resolver(policy, Arc::new(SystemResolver))
    }

    /// A guard that uses a custom resolver.
    pub fn with_resolver(policy: SsrfPolicy, resolver: Arc<dyn HostResolver>) -> Self {
        Self { policy, resolver }
    }

    /// Full check used at registration: parse the URL, then check the IP
    /// literal or every address the host resolves to.
    pub async fn validate_url(&self, raw: &str) -> Result<reqwest::Url, TargetError> {
        let (url, host) = parse_target(raw)?;
        match host {
            TargetHost::Ip(ip) => self.check_ip(&url_host(&url), ip)?,
            TargetHost::Domain(domain) => {
                self.resolve_checked(&domain).await?;
            }
        }
        Ok(url)
    }

    /// Check used at delivery, before the request is sent. It covers IP
    /// literals, which never reach the resolver. Hostnames are checked by
    /// [`GuardedResolver`] when the connection is opened.
    pub fn check_literal(&self, raw: &str) -> Result<(), TargetError> {
        let (url, host) = parse_target(raw)?;
        if let TargetHost::Ip(ip) = host {
            self.check_ip(&url_host(&url), ip)?;
        }
        Ok(())
    }

    /// Resolve `host` and fail if it has no addresses or any address is
    /// blocked. One blocked address fails the whole host, so a record mixing
    /// public and private addresses cannot be used to get through.
    pub async fn resolve_checked(&self, host: &str) -> Result<Vec<IpAddr>, TargetError> {
        let ips = self
            .resolver
            .lookup(host)
            .await
            .map_err(|err| TargetError::Unresolvable {
                host: host.to_string(),
                reason: err.to_string(),
            })?;
        if ips.is_empty() {
            return Err(TargetError::Unresolvable {
                host: host.to_string(),
                reason: "no addresses returned".to_string(),
            });
        }
        for ip in &ips {
            self.check_ip(host, *ip)?;
        }
        Ok(ips)
    }

    fn check_ip(&self, host: &str, ip: IpAddr) -> Result<(), TargetError> {
        if self.policy.permits(ip) {
            Ok(())
        } else {
            Err(TargetError::BlockedAddress {
                host: host.to_string(),
                ip,
            })
        }
    }

    /// Build the HTTP client used for webhook deliveries. Every connection
    /// goes through [`GuardedResolver`]. Redirects and proxies are turned off
    /// because both could send the request to a host that was never checked.
    pub fn build_client(&self, timeout: Duration) -> reqwest::Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .dns_resolver(Arc::new(GuardedResolver {
                guard: self.clone(),
            }))
            .build()
    }
}

/// reqwest DNS resolver that refuses blocked addresses. reqwest connects only
/// to the addresses returned here, so the address that was checked is the
/// address that gets used.
pub struct GuardedResolver {
    guard: WebhookTargetGuard,
}

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let guard = self.guard.clone();
        let host = name.as_str().to_string();
        Box::pin(async move {
            let ips = guard
                .resolve_checked(&host)
                .await
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?;
            // Port 0 is replaced by the URL's port (or the scheme default).
            let addrs: reqwest::dns::Addrs =
                Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}

/// Find a [`TargetError`] in an error's source chain. reqwest wraps resolver
/// errors several layers deep.
pub fn target_error_in_chain(err: &(dyn std::error::Error + 'static)) -> Option<TargetError> {
    let mut current = Some(err);
    while let Some(e) = current {
        if let Some(target) = e.downcast_ref::<TargetError>() {
            return Some(target.clone());
        }
        current = e.source();
    }
    None
}

enum TargetHost {
    Ip(IpAddr),
    Domain(String),
}

/// Parse and syntax-check a webhook URL. The WHATWG parser normalises odd
/// IPv4 spellings such as `http://2130706433/` or `http://0x7f.1/` into
/// dotted form, so they are caught as IP literals.
fn parse_target(raw: &str) -> Result<(reqwest::Url, TargetHost), TargetError> {
    let url =
        reqwest::Url::parse(raw.trim()).map_err(|e| TargetError::InvalidUrl(e.to_string()))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(TargetError::UnsupportedScheme(other.to_string())),
    }
    let host = url_host(&url);
    if host.is_empty() {
        return Err(TargetError::MissingHost);
    }
    let target = match host.parse::<IpAddr>() {
        Ok(ip) => TargetHost::Ip(ip),
        Err(_) => TargetHost::Domain(host),
    };
    Ok((url, target))
}

/// The URL host without the brackets around IPv6 literals.
fn url_host(url: &reqwest::Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string()
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    /// Fake resolver. Each host has a queue of answers; the last answer is
    /// repeated once the queue is down to one. Queuing a public address and
    /// then a private one simulates DNS rebinding.
    #[derive(Default)]
    pub struct MockResolver {
        answers: Mutex<HashMap<String, VecDeque<Vec<IpAddr>>>>,
        pub lookups: Mutex<Vec<String>>,
    }

    impl MockResolver {
        pub fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        pub fn answer(self: &Arc<Self>, host: &str, ips: &[&str]) -> Arc<Self> {
            let parsed = ips.iter().map(|ip| ip.parse().unwrap()).collect();
            self.answers
                .lock()
                .unwrap()
                .entry(host.to_string())
                .or_default()
                .push_back(parsed);
            self.clone()
        }

        pub fn lookup_count(&self, host: &str) -> usize {
            self.lookups
                .lock()
                .unwrap()
                .iter()
                .filter(|h| *h == host)
                .count()
        }
    }

    impl HostResolver for MockResolver {
        fn lookup(&self, host: &str) -> LookupFuture {
            self.lookups.lock().unwrap().push(host.to_string());
            let mut answers = self.answers.lock().unwrap();
            let result = match answers.get_mut(host) {
                Some(queue) if queue.len() > 1 => Ok(queue.pop_front().unwrap()),
                Some(queue) => Ok(queue.front().cloned().unwrap_or_default()),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no mock answer for {host}"),
                )),
            };
            Box::pin(async move { result })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::MockResolver;
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn blocks_private_loopback_link_local_and_metadata_addresses() {
        for addr in [
            "0.0.0.0",
            "0.1.2.3",
            "127.0.0.1",
            "127.255.255.254",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254", // AWS / GCP / Azure metadata
            "169.254.0.1",
            "100.64.0.1",
            "100.100.100.200", // Alibaba Cloud metadata
            "192.0.0.192",
            "198.18.0.1",
            "192.0.2.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fe80::1",
            "fc00::1",
            "fd00:ec2::254", // AWS metadata over IPv6
            "fec0::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.0.0.1",
            "::127.0.0.1",
            "64:ff9b::a00:1", // NAT64 of 10.0.0.1
            "2002:a00:1::1",  // 6to4 of 10.0.0.1
            "2001:0:4136::1", // Teredo
        ] {
            assert!(is_blocked_ip(ip(addr)), "{addr} must be blocked");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for addr in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.215.14",
            "172.15.255.255",
            "172.32.0.1",
            "100.63.255.255",
            "100.128.0.1",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808", // NAT64 of 8.8.8.8
        ] {
            assert!(!is_blocked_ip(ip(addr)), "{addr} must be allowed");
        }
    }

    #[test]
    fn override_flag_parsing() {
        assert!(!SsrfPolicy::from_flag(None).allow_private_targets);
        assert!(!SsrfPolicy::from_flag(Some("")).allow_private_targets);
        assert!(!SsrfPolicy::from_flag(Some("false")).allow_private_targets);
        assert!(!SsrfPolicy::from_flag(Some("0")).allow_private_targets);
        assert!(SsrfPolicy::from_flag(Some("true")).allow_private_targets);
        assert!(SsrfPolicy::from_flag(Some(" TRUE ")).allow_private_targets);
        assert!(SsrfPolicy::from_flag(Some("1")).allow_private_targets);
        assert!(SsrfPolicy::from_flag(Some("yes")).allow_private_targets);
        assert!(SsrfPolicy::allow_private().permits(ip("127.0.0.1")));
        assert!(!SsrfPolicy::strict().permits(ip("127.0.0.1")));
    }

    #[tokio::test]
    async fn rejects_ip_literal_targets_including_obfuscated_forms() {
        let guard = WebhookTargetGuard::with_resolver(SsrfPolicy::strict(), MockResolver::new());
        for url in [
            "http://127.0.0.1/hook",
            "http://127.0.0.1:8080/hook",
            "https://10.1.2.3/hook",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/hook",
            "http://[::ffff:127.0.0.1]/hook",
            "http://[fd00:ec2::254]/",
            "http://2130706433/", // 127.0.0.1 as a decimal integer
            "http://0x7f000001/", // 127.0.0.1 in hex
            "http://0177.0.0.1/", // 127.0.0.1 in octal
            "http://127.1/",      // shortened form
        ] {
            let err = guard.validate_url(url).await.unwrap_err();
            assert!(
                matches!(err, TargetError::BlockedAddress { .. }),
                "{url} must be blocked, got {err:?}"
            );
            assert!(
                guard.check_literal(url).is_err(),
                "{url} must fail check_literal"
            );
        }
    }

    #[tokio::test]
    async fn rejects_bad_schemes_and_malformed_urls() {
        let guard = WebhookTargetGuard::with_resolver(SsrfPolicy::strict(), MockResolver::new());
        assert!(matches!(
            guard.validate_url("ftp://example.com/").await,
            Err(TargetError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            guard.validate_url("file:///etc/passwd").await,
            Err(TargetError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            guard.validate_url("not a url").await,
            Err(TargetError::InvalidUrl(_))
        ));
        assert!(matches!(
            guard.validate_url("http://").await,
            Err(TargetError::InvalidUrl(_))
        ));
    }

    #[tokio::test]
    async fn rejects_hostnames_resolving_to_blocked_addresses() {
        let resolver = MockResolver::new()
            .answer("internal.example", &["10.0.0.7"])
            .answer("mixed.example", &["93.184.215.14", "192.168.0.10"])
            .answer("metadata.example", &["169.254.169.254"]);
        let guard = WebhookTargetGuard::with_resolver(SsrfPolicy::strict(), resolver);

        for url in [
            "https://internal.example/hook",
            "https://mixed.example/hook",
            "http://metadata.example/",
        ] {
            assert!(
                matches!(
                    guard.validate_url(url).await,
                    Err(TargetError::BlockedAddress { .. })
                ),
                "{url} must be blocked"
            );
        }

        assert!(matches!(
            guard.validate_url("https://unknown.example/").await,
            Err(TargetError::Unresolvable { .. })
        ));
    }

    #[tokio::test]
    async fn allows_public_targets_and_private_ones_with_override() {
        let resolver = MockResolver::new()
            .answer(
                "hooks.example",
                &["93.184.215.14", "2606:2800:21f:cb07:6820:80da:af6b:8b2c"],
            )
            .answer("localdev.example", &["127.0.0.1"]);
        let strict = WebhookTargetGuard::with_resolver(SsrfPolicy::strict(), resolver.clone());
        assert!(strict
            .validate_url("https://hooks.example/receiver")
            .await
            .is_ok());
        assert!(strict
            .validate_url("https://8.8.8.8/receiver")
            .await
            .is_ok());
        assert!(strict
            .check_literal("https://hooks.example/receiver")
            .is_ok());

        let dev = WebhookTargetGuard::with_resolver(SsrfPolicy::allow_private(), resolver);
        assert!(dev
            .validate_url("http://localdev.example:3000/hook")
            .await
            .is_ok());
        assert!(dev.validate_url("http://127.0.0.1:3000/hook").await.is_ok());
        assert!(dev.check_literal("http://[::1]:3000/hook").is_ok());
    }

    #[test]
    fn target_error_is_found_through_wrapped_sources() {
        #[derive(Debug)]
        struct Wrapper(TargetError);
        impl std::fmt::Display for Wrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "wrapped")
            }
        }
        impl std::error::Error for Wrapper {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        let inner = TargetError::MissingHost;
        let wrapped = Wrapper(inner.clone());
        assert_eq!(target_error_in_chain(&wrapped), Some(inner));
        assert_eq!(target_error_in_chain(&std::fmt::Error), None);
    }
}
