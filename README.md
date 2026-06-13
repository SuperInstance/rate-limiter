# Rate Limiter: Multi-Algorithm Traffic Control

**Rate limiting** controls the rate at which requests are processed, preventing overload and ensuring fair resource allocation. This crate implements three industry-standard algorithms — **token bucket**, **sliding window counter**, and **leaky bucket** — with thread-safe wrappers and a runnable demonstration of each strategy.

## Why It Matters

Every production system needs rate limiting: APIs (prevent abuse), databases (prevent connection exhaustion), message queues (prevent backpressure floods), and networks (prevent congestion collapse). Different scenarios call for different algorithms. Token bucket allows bursts; sliding window enforces strict limits; leaky bucket smooths output to a steady rate. Having all three in one crate lets you choose the right tool without switching dependencies.

## How It Works

### Token Bucket

```
Tokens accumulate at `refill_rate` per second.
Each request consumes 1 token.
Burst capacity = max_tokens.

tokens(t) = min(max_tokens, tokens(t₀) + (t − t₀) × refill_rate)
```

**Best for:** API gateways, outbound request shaping.

### Sliding Window Counter

```
Maintains timestamps of all requests in the window [t − W, t].
Request allowed if count < max_requests.

Eviction on each call:
  while timestamps.last() < t − W:
    timestamps.pop_back()
```

**Best for:** Strict per-second/per-minute limits with zero burst tolerance.

### Leaky Bucket

```
Water level rises with each request, leaks at constant rate.
Requests are accepted if water < capacity.
Requests are dropped (overflow) if water = capacity.

leak_amount = elapsed_ms / leak_rate_ms
water = max(0, water − leak_amount)
```

**Best for:** Output smoothing — converting bursty input into steady output.

### Mathematical Comparison

```
Input: 100 requests in 1 second, then idle

Token Bucket (max=10, refill=5/s):
  t=0:   10 allowed, 90 rejected
  t=1s:  5 more available
  t=2s:  10 available (full)

Sliding Window (max=10, window=1s):
  t=0:   10 allowed, 90 rejected
  t=1s:  window resets → 10 available

Leaky Bucket (cap=5, leak=100ms):
  t=0:   5 accepted, 95 overflow
  Water drains steadily: 5 → 0 in 500ms
```

### Thread Safety

Both `ThreadSafeTokenBucket` and `ThreadSafeSlidingWindow` wrap their inner limiter in `Mutex`, enabling safe concurrent access from multiple threads. The token bucket's O(1) per operation makes it ideal for high-contention scenarios.

**Complexity:**

| Algorithm | Time | Space | Notes |
|-----------|------|-------|-------|
| Token Bucket | O(1) | O(1) | Lazy refill |
| Sliding Window | O(E) amortized | O(R) | E = evictions |
| Leaky Bucket | O(1) | O(1) | Integer arithmetic |

## Quick Start

```rust
use rate_limiter::{
    TokenBucketRateLimiter,
    SlidingWindowRateLimiter,
    ThreadSafeTokenBucket,
};
use std::time::Duration;
use std::sync::Arc;
use std::thread;

fn main() {
    // === Token Bucket ===
    let mut bucket = TokenBucketRateLimiter::new(10, 5.0); // 10 max, 5/sec refill
    for i in 0..15 {
        let ok = bucket.try_acquire();
        println!("Token bucket req {}: {}", i + 1, if ok { "✓" } else { "✗" });
    }

    // === Sliding Window ===
    let mut sw = SlidingWindowRateLimiter::new(Duration::from_secs(1), 5);
    for i in 0..8 {
        let ok = sw.try_acquire();
        println!("Sliding window req {}: {}", i + 1, if ok { "✓" } else { "✗" });
    }

    // === Concurrent Token Bucket ===
    let bucket = Arc::new(ThreadSafeTokenBucket::new(100, 50.0));
    let mut handles = vec![];
    for _ in 0..4 {
        let b = Arc::clone(&bucket);
        handles.push(thread::spawn(move || {
            let mut acquired = 0;
            for _ in 0..30 {
                if b.try_acquire() { acquired += 1; }
            }
            acquired
        }));
    }
    let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    println!("Concurrent: {} tokens consumed", total);
}
```

## API

### Token Bucket
- `TokenBucketRateLimiter::new(max_tokens, refill_per_second) -> Self`
- `try_acquire() -> bool` — Consume one token.
- `available() -> u32` — Current token count (after refill).
- `refill()` — Force token computation.

### Sliding Window
- `SlidingWindowRateLimiter::new(window, max_requests) -> Self`
- `try_acquire() -> bool` — Record request, return if allowed.

### Thread-Safe Wrappers
- `ThreadSafeTokenBucket` — `Mutex<TokenBucketRateLimiter>`
- `ThreadSafeSlidingWindow` — `Mutex<SlidingWindowRateLimiter>`

## Architecture Notes

In SuperInstance, `rate-limiter` is the unified rate-limiting crate providing all three algorithms behind a single dependency. The gateway layer selects the algorithm per route: token bucket for general APIs, sliding window for authentication, and leaky bucket for database connection pooling. In the **γ + η = C** model, rate limiting protects both γ (by preventing overload-induced failures) and η (by smoothing traffic).

See [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

## References

1. Turner, J. *New Directions in Communications*. IEEE Communications, 1986.
2. Redis Labs. *Rate Limiting: Algorithms and Patterns*. [redis.com/glossary/rate-limiting](https://redis.com/glossary/rate-limiting/)
3. Cloudflare. *Understanding Rate Limiting: Algorithms Compared*. Cloudflare Learning Center.

## License

MIT
