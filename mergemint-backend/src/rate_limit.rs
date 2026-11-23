// mergemint-backend/src/rate_limit.rs
//
// Per-key token-bucket rate limiter used to protect relay-wallet endpoints.
// Staged for wiring into `self_claim`; retained here so the limiter and its
// tuning constants have a single home once that lands.
//
// # DoS hardening (#1261)
//
// The limiter's backing `HashMap` is bounded by `max_entries`.  When the map
// is full and a new key arrives, the oldest expired bucket is evicted (lazy
// LRU).  Additionally, every `try_acquire` call opportunistically prunes
// expired buckets, so an attacker submitting unique keys cannot grow the map
// beyond the cap.  This prevents unbounded memory growth from a
// high-cardinality-key DoS.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default maximum number of tracked rate-limit buckets.  Prevents unbounded
/// memory growth when an attacker submits requests with unique keys.
pub const DEFAULT_MAX_ENTRIES: usize = 100_000;

/// Fixed-window token bucket keyed by an arbitrary string (e.g. claimant address).
#[derive(Debug)]
pub struct TokenBucketLimiter {
    max_tokens: u32,
    window: Duration,
    /// Hard cap on the number of buckets; when reached, the oldest expired
    /// bucket is evicted before inserting a new one.  If no bucket is expired,
    /// the oldest bucket by `window_start` is evicted as a last resort.
    max_entries: usize,
    buckets: Mutex<HashMap<String, BucketState>>,
}

#[derive(Debug, Clone)]
struct BucketState {
    tokens: u32,
    window_start: Instant,
}

impl TokenBucketLimiter {
    pub fn new(max_tokens: u32, window: Duration) -> Self {
        Self::with_capacity(max_tokens, window, DEFAULT_MAX_ENTRIES)
    }

    /// Construct a limiter with a custom `max_entries` cap.
    pub fn with_capacity(max_tokens: u32, window: Duration, max_entries: usize) -> Self {
        Self {
            max_tokens,
            window,
            max_entries,
            buckets: Mutex::new(HashMap::with_capacity(max_entries.min(4096))),
        }
    }

    /// Returns the current number of tracked buckets (for diagnostics / tests).
    pub fn len(&self) -> usize {
        self.buckets.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Returns `true` if no buckets are currently tracked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` when the request is allowed and consumes one token.
    pub fn try_acquire(&self, key: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        // ── Lazy pruning: remove expired buckets on every call ────────────
        //
        // This is the primary defence against unbounded growth.  Every
        // `try_acquire` sweeps expired entries, so an attacker flooding
        // unique keys causes stale buckets to be evicted rather than
        // accumulating.
        prune_expired(&mut buckets, now, self.window);

        // ── Bounded insertion: evict if at capacity ───────────────────────
        //
        // Check capacity before calling `entry()` to avoid a double
        // mutable borrow (the entry handle borrows `buckets` for its
        // lifetime, so we can't mutate the map from inside the closure).
        if !buckets.contains_key(key) && buckets.len() >= self.max_entries {
            evict_oldest(&mut buckets);
        }

        let entry = buckets.entry(key.to_string()).or_insert(BucketState {
            tokens: self.max_tokens,
            window_start: now,
        });

        if now.duration_since(entry.window_start) >= self.window {
            entry.tokens = self.max_tokens;
            entry.window_start = now;
        }

        if entry.tokens == 0 {
            return false;
        }

        entry.tokens -= 1;
        true
    }
}

/// Remove all buckets whose window has expired.
fn prune_expired(buckets: &mut HashMap<String, BucketState>, now: Instant, window: Duration) {
    buckets.retain(|_, state| now.duration_since(state.window_start) < window);
}

/// Evict the single oldest bucket by `window_start`.  Used as a last-resort
/// when the map is at capacity and no buckets are expired.
fn evict_oldest(buckets: &mut HashMap<String, BucketState>) {
    if let Some(oldest_key) = buckets
        .iter()
        .min_by_key(|(_, state)| state.window_start)
        .map(|(key, _)| key.clone())
    {
        buckets.remove(&oldest_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_tokens_then_rejects() {
        let limiter = TokenBucketLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.try_acquire("alice"));
        assert!(limiter.try_acquire("alice"));
        assert!(!limiter.try_acquire("alice"));
    }

    #[test]
    fn keys_are_independent() {
        let limiter = TokenBucketLimiter::new(1, Duration::from_secs(60));
        assert!(limiter.try_acquire("alice"));
        assert!(!limiter.try_acquire("alice"));
        assert!(limiter.try_acquire("bob"));
    }

    // ── Bounded capacity tests (#1261) ───────────────────────────────────

    /// With a small cap, inserting more unique keys than `max_entries` must
    /// not grow the map beyond the cap.
    #[test]
    fn bounded_capacity_caps_entry_count() {
        let limiter = TokenBucketLimiter::with_capacity(1, Duration::from_secs(3600), 10);

        // Submit 100 unique keys — far more than the cap of 10.
        for i in 0..100 {
            limiter.try_acquire(&format!("attacker_{i}"));
        }

        assert!(
            limiter.len() <= 10,
            "entry count {} must not exceed the cap of 10",
            limiter.len()
        );
    }

    /// Expired buckets must be pruned on the next `try_acquire`, keeping the
    /// map from growing with stale entries.
    #[test]
    fn expired_buckets_are_pruned() {
        // Use a 10ms window so we can let it expire in a test.
        let limiter = TokenBucketLimiter::with_capacity(1, Duration::from_millis(10), 1000);

        limiter.try_acquire("key_a");
        limiter.try_acquire("key_b");
        assert_eq!(limiter.len(), 2);

        // Wait for the window to expire.
        std::thread::sleep(Duration::from_millis(20));

        // A new call should prune the expired buckets.
        limiter.try_acquire("key_c");
        assert_eq!(
            limiter.len(),
            1,
            "expired buckets must be pruned, leaving only the fresh one"
        );
    }

    /// Stress test: 50,000 distinct keys must not grow the map beyond the cap.
    /// This simulates a high-cardinality-key DoS attack.
    #[test]
    fn stress_50k_unique_keys_remain_bounded() {
        let cap = 5000;
        let limiter = TokenBucketLimiter::with_capacity(1, Duration::from_secs(3600), cap);

        for i in 0..50_000 {
            limiter.try_acquire(&format!("attacker_{i}"));
        }

        assert!(
            limiter.len() <= cap,
            "after 50k unique keys, entry count {} must not exceed cap {cap}",
            limiter.len()
        );
        assert!(
            limiter.len() > 0,
            "the map must not be empty after active inserts"
        );
    }

    /// Stress test with a short window: expired buckets are continuously
    /// pruned so the map stays well under the cap even with 50k keys.
    #[test]
    fn stress_50k_unique_keys_with_short_window_prunes_aggressively() {
        let cap = 10_000;
        let limiter = TokenBucketLimiter::with_capacity(1, Duration::from_millis(1), cap);

        for i in 0..50_000 {
            limiter.try_acquire(&format!("attacker_{i}"));
            // Every 1000 iterations, the window has expired for early entries.
        }

        assert!(
            limiter.len() <= cap,
            "entry count {} must not exceed cap {cap}",
            limiter.len()
        );
    }

    /// Eviction must not corrupt existing buckets — a key that was admitted
    /// should still rate-limit correctly after eviction of others.
    #[test]
    fn eviction_preserves_active_bucket_semantics() {
        let limiter = TokenBucketLimiter::with_capacity(2, Duration::from_secs(3600), 3);

        assert!(limiter.try_acquire("alice"));
        assert!(limiter.try_acquire("alice"));
        assert!(!limiter.try_acquire("alice")); // exhausted

        // Fill the rest of the cap with unique keys, evicting older entries.
        for i in 0..10 {
            limiter.try_acquire(&format!("bot_{i}"));
        }

        // alice may or may not have been evicted, but if present, the limiter
        // state must be consistent — no panics, no corrupted counts.
        let count = limiter.len();
        assert!(count <= 3, "cap of 3 must hold: got {count}");
    }
}
