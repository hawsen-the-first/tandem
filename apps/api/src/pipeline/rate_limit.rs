//! Per-source token buckets (LC-7, SE-12, BR-3.6). In memory per process and bounded.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Which allowance a route draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Ordinary requests: 300 per minute.
    General,
    /// Sign-in and token endpoints: 20 per minute.
    Auth,
    /// First-run setup: 10 per minute.
    Setup,
    /// Starting a configuration test: 10 per minute.
    Test,
}

impl Bucket {
    /// The label used in `rate_limited_total{bucket}`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Auth => "auth",
            Self::Setup => "setup",
            Self::Test => "test",
        }
    }

    const fn per_minute(self) -> f64 {
        match self {
            Self::General => 300.0,
            Self::Auth => 20.0,
            Self::Setup => 10.0,
            Self::Test => 10.0,
        }
    }
}

/// The answer to one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead.
    Allowed,
    /// Refused; try again after this long.
    Retry(Duration),
}

#[derive(Debug, Clone, Copy)]
struct State {
    tokens: f64,
    last: Instant,
}

/// The limiter. Memory is bounded: past `max_entries` the least recently used entry is evicted.
#[derive(Debug)]
pub struct RateLimiter {
    entries: Mutex<HashMap<(Bucket, IpAddr), State>>,
    max_entries: usize,
}

impl RateLimiter {
    /// A limiter holding at most `max_entries` sources.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries: max_entries.max(1),
        }
    }

    /// Take one token for `source` from `bucket` at time `now`.
    pub fn check(&self, source: IpAddr, bucket: Bucket, now: Instant) -> Decision {
        let capacity = bucket.per_minute();
        let refill_per_second = capacity / 60.0;
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.len() >= self.max_entries
            && !entries.contains_key(&(bucket, source))
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, state)| state.last)
                .map(|(key, _)| *key)
        {
            entries.remove(&oldest);
        }
        let state = entries.entry((bucket, source)).or_insert(State {
            tokens: capacity,
            last: now,
        });
        let elapsed = now.saturating_duration_since(state.last).as_secs_f64();
        state.tokens = (state.tokens + elapsed * refill_per_second).min(capacity);
        state.last = now;
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Decision::Allowed
        } else {
            let wait = (1.0 - state.tokens) / refill_per_second;
            Decision::Retry(Duration::from_secs_f64(wait))
        }
    }

    /// How many sources are tracked (for tests and metrics).
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Whether nothing is tracked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
