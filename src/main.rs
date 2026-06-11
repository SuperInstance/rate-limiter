use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
}

struct SlidingWindowCounter {
    window: Duration,
    max_requests: u32,
    timestamps: VecDeque<Instant>,
}

impl SlidingWindowCounter {
    fn new(window: Duration, max_requests: u32) -> Self {
        Self {
            window,
            max_requests,
            timestamps: VecDeque::new(),
        }
    }

    fn try_request(&mut self) -> bool {
        let now = Instant::now();
        let cutoff = now - self.window;
        while self.timestamps.back().map_or(false, |t| *t < cutoff) {
            self.timestamps.pop_back();
        }
        if self.timestamps.len() < self.max_requests as usize {
            self.timestamps.push_front(now);
            true
        } else {
            false
        }
    }

    fn remaining(&self) -> u32 {
        self.max_requests.saturating_sub(self.timestamps.len() as u32)
    }
}

struct LeakyBucket {
    capacity: u32,
    water: u32,
    leak_rate: Duration, // time per unit leaked
    last_leak: Instant,
}

impl LeakyBucket {
    fn new(capacity: u32, leak_rate: Duration) -> Self {
        Self { capacity, water: 0, leak_rate, last_leak: Instant::now() }
    }

    fn try_add(&mut self) -> bool {
        self.leak();
        if self.water < self.capacity {
            self.water += 1;
            true
        } else {
            false
        }
    }

    fn leak(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_leak);
        let leaked = elapsed.as_millis() / self.leak_rate.as_millis();
        self.water = self.water.saturating_sub(leaked as u32);
        self.last_leak = now;
    }
}

fn main() {
    // Token bucket demo
    let mut bucket = TokenBucket::new(10.0, 5.0);
    println!("Token Bucket (max=10, refill=5/s):");
    for i in 0..15 {
        let ok = bucket.try_consume(1.0);
        println!("  Request {}: {}", i + 1, if ok { "allowed" } else { "rejected" });
    }

    // Sliding window demo
    let mut window = SlidingWindowCounter::new(Duration::from_secs(1), 5);
    println!("\nSliding Window (5 req/s):");
    for i in 0..8 {
        let ok = window.try_request();
        println!("  Request {}: {} (remaining: {})", i + 1,
            if ok { "allowed" } else { "rejected" }, window.remaining());
    }

    // Leaky bucket demo
    let mut leaky = LeakyBucket::new(5, Duration::from_millis(100));
    println!("\nLeaky Bucket (capacity=5):");
    for i in 0..7 {
        let ok = leaky.try_add();
        println!("  Add {}: {} (water: {})", i + 1,
            if ok { "accepted" } else { "overflow" }, leaky.water);
    }
}
