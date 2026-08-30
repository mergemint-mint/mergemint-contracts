// mergemint-backend/src/rate_limit.rs
//
// Per-key token-bucket rate limiter used to protect relay-wallet endpoints.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Fixed-window token bucket keyed by an arbitrary string (e.g. claimant address).
#[derive(Debug)]
pub struct TokenBucketLimiter {
    max_tokens: u32,
    window: Duration,
    buckets: Mutex<HashMap<String, BucketState>>,
}

#[derive(Debug, Clone)]
struct BucketState {
    tokens: u32,
    window_start: Instant,
}

impl TokenBucketLimiter {
    pub fn new(max_tokens: u32, window: Duration) -> Self {
        Self {
            max_tokens,
            window,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` when the request is allowed and consumes one token.
    pub fn try_acquire(&self, key: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
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
}
