//! Rate limiter implementations: token bucket and sliding window counter.
//!
//! Provides two rate-limiting strategies suitable for API management,
//! traffic shaping, and resource protection.

use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Token Bucket
// ---------------------------------------------------------------------------

/// Configuration for creating a [`TokenBucketRateLimiter`].
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    pub max_tokens: u32,
    pub refill_per_second: f64,
}

/// Token-bucket rate limiter.
///
/// Tokens are consumed on each `try_acquire` and replenished at a steady rate
/// proportional to elapsed time, up to `max_tokens`.
pub struct TokenBucketRateLimiter {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucketRateLimiter {
    /// Create a new token bucket that holds up to `max_tokens` and refills at
    /// `refill_per_second` tokens per second. The bucket starts full.
    pub fn new(max_tokens: u32, refill_per_second: f64) -> Self {
        assert!(max_tokens > 0, "max_tokens must be > 0");
        assert!(refill_per_second > 0.0, "refill_per_second must be > 0");
        Self {
            tokens: max_tokens as f64,
            max_tokens: max_tokens as f64,
            refill_rate: refill_per_second,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time since last refill.
    pub fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let added = elapsed * self.refill_rate;
        self.tokens = (self.tokens + added).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Try to acquire one token. Returns `true` if allowed.
    pub fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Alias for [`try_acquire`].
    pub fn acquire(&mut self) -> bool {
        self.try_acquire()
    }

    /// Returns the current number of available tokens (floor).
    pub fn available(&mut self) -> u32 {
        self.refill();
        self.tokens.floor() as u32
    }
}

// ---------------------------------------------------------------------------
// Sliding Window Counter
// ---------------------------------------------------------------------------

/// Sliding-window rate limiter.
///
/// Tracks per-request timestamps within a rolling time window of length
/// `window`. Rejects requests once `max_requests` entries fall within the
/// window.
pub struct SlidingWindowRateLimiter {
    window: Duration,
    max_requests: u32,
    timestamps: Vec<Instant>,
}

impl SlidingWindowRateLimiter {
    /// Create a new sliding window limiter.
    pub fn new(window: Duration, max_requests: u32) -> Self {
        assert!(max_requests > 0, "max_requests must be > 0");
        Self {
            window,
            max_requests,
            timestamps: Vec::with_capacity(max_requests as usize),
        }
    }

    /// Evict timestamps that have fallen outside the window.
    fn evict(&mut self) {
        let cutoff = Instant::now() - self.window;
        // Keep only timestamps >= cutoff (drain filter via retain)
        self.timestamps.retain(|&ts| ts > cutoff);
    }

    /// Try to acquire one request slot. Returns `true` if allowed.
    pub fn try_acquire(&mut self) -> bool {
        self.evict();
        if self.timestamps.len() < self.max_requests as usize {
            self.timestamps.push(Instant::now());
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Thread-safe wrappers
// ---------------------------------------------------------------------------

/// Thread-safe token bucket wrapped in a `Mutex`.
pub struct ThreadSafeTokenBucket {
    inner: Mutex<TokenBucketRateLimiter>,
}

impl ThreadSafeTokenBucket {
    pub fn new(max_tokens: u32, refill_per_second: f64) -> Self {
        Self {
            inner: Mutex::new(TokenBucketRateLimiter::new(max_tokens, refill_per_second)),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.inner.lock().unwrap().try_acquire()
    }

    pub fn available(&self) -> u32 {
        self.inner.lock().unwrap().available()
    }
}

/// Thread-safe sliding window wrapped in a `Mutex`.
pub struct ThreadSafeSlidingWindow {
    inner: Mutex<SlidingWindowRateLimiter>,
}

impl ThreadSafeSlidingWindow {
    pub fn new(window: Duration, max_requests: u32) -> Self {
        Self {
            inner: Mutex::new(SlidingWindowRateLimiter::new(window, max_requests)),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.inner.lock().unwrap().try_acquire()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    // -- Token bucket tests --

    #[test]
    fn token_bucket_allows_up_to_max() {
        let mut rl = TokenBucketRateLimiter::new(5, 1.0);
        for _ in 0..5 {
            assert!(rl.try_acquire(), "should allow within max");
        }
    }

    #[test]
    fn token_bucket_rejects_when_empty() {
        let mut rl = TokenBucketRateLimiter::new(3, 1.0);
        for _ in 0..3 {
            assert!(rl.try_acquire());
        }
        assert!(!rl.try_acquire(), "should reject when bucket is empty");
    }

    #[test]
    fn token_bucket_refills_over_time() {
        // 2 tokens per second refill, max 10
        let mut rl = TokenBucketRateLimiter::new(10, 1000.0); // fast refill
        // Drain all
        for _ in 0..10 {
            assert!(rl.try_acquire());
        }
        assert!(!rl.try_acquire());
        // Wait a bit for refill — 10ms at 1000/s = ~10 tokens
        thread::sleep(Duration::from_millis(15));
        assert!(rl.try_acquire(), "should have refilled");
    }

    // -- Sliding window tests --

    #[test]
    fn sliding_window_allows_within_window() {
        let mut rl = SlidingWindowRateLimiter::new(Duration::from_secs(60), 5);
        for _ in 0..5 {
            assert!(rl.try_acquire(), "should allow within limit");
        }
    }

    #[test]
    fn sliding_window_rejects_over_limit() {
        let mut rl = SlidingWindowRateLimiter::new(Duration::from_secs(60), 3);
        for _ in 0..3 {
            assert!(rl.try_acquire());
        }
        assert!(!rl.try_acquire(), "should reject over limit");
    }

    #[test]
    fn sliding_window_resets_after_window_expires() {
        let mut rl = SlidingWindowRateLimiter::new(Duration::from_millis(50), 2);
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(!rl.try_acquire(), "should reject at limit");
        // Wait for window to expire
        thread::sleep(Duration::from_millis(60));
        assert!(rl.try_acquire(), "should allow after window expires");
    }

    // -- Concurrency test --

    #[test]
    fn concurrent_access_safe() {
        let bucket = Arc::new(ThreadSafeTokenBucket::new(100, 100.0));
        let mut handles = vec![];

        for _ in 0..10 {
            let b = Arc::clone(&bucket);
            handles.push(thread::spawn(move || {
                let mut acquired = 0u32;
                for _ in 0..15 {
                    if b.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        // At most 100 tokens were available; some may have refilled during the test
        // so we just assert it doesn't panic and total is reasonable.
        assert!(total >= 100, "at least the initial tokens should be consumed");
    }

    #[test]
    fn thread_safe_sliding_window_concurrent() {
        let sw = Arc::new(ThreadSafeSlidingWindow::new(Duration::from_secs(60), 50));
        let mut handles = vec![];

        for _ in 0..10 {
            let s = Arc::clone(&sw);
            handles.push(thread::spawn(move || {
                let mut acquired = 0u32;
                for _ in 0..10 {
                    if s.try_acquire() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 50, "exactly max_requests should be allowed");
    }
}
