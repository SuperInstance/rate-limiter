# SEED-SPEC: rate-limiter

## Purpose
Token bucket and sliding window rate limiting for API management. Essential for protecting fleet services from overload and enforcing usage quotas.

## API Sketch
```rust
pub trait RateLimiter: Send + Sync {
    async fn acquire(&self, key: &str) -> Result<RateLimitDecision, RateLimitError>;
    async fn acquire_n(&self, key: &str, n: u32) -> Result<RateLimitDecision, RateLimitError>;
}

pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub reset_at: Instant,
    pub retry_after: Option<Duration>,
}

pub struct TokenBucketConfig {
    pub capacity: u32,
    pub refill_rate: u32,     // tokens per second
}

pub struct SlidingWindowConfig {
    pub limit: u32,
    pub window: Duration,
}
```

## Dependencies
- `tokio` (async runtime, time)
- `fleet-metrics` (rate limit counters)
- `serde` (config)

## Status: SEED (not yet implemented)
