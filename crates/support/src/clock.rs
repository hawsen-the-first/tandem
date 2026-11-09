//! Time source abstraction.

use std::fmt::Debug;
use std::sync::{Arc, Mutex, PoisonError};
use time::{Duration, OffsetDateTime};

/// A source of the current UTC time. Business code never calls the system clock directly.
pub trait Clock: Send + Sync + Debug {
    /// The current UTC time.
    fn now(&self) -> OffsetDateTime;
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// A clock that only moves when told to. Clones share the same time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<Mutex<OffsetDateTime>>,
}

impl FakeClock {
    /// A fake clock starting at the given instant.
    pub fn new(start: OffsetDateTime) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    /// A fake clock starting at the given Unix timestamp in seconds (out-of-range values clamp to
    /// the Unix epoch).
    pub fn at_unix(seconds: i64) -> Self {
        Self::new(
            OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(OffsetDateTime::UNIX_EPOCH),
        )
    }

    /// Move time forward (or backward with a negative duration).
    pub fn advance(&self, by: Duration) {
        let mut guard = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *guard += by;
    }

    /// Set the time.
    pub fn set(&self, to: OffsetDateTime) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = to;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_moves_only_when_advanced_and_clones_share_time() {
        let clock = FakeClock::at_unix(1_000);
        let other = clock.clone();
        assert_eq!(clock.now().unix_timestamp(), 1_000);
        other.advance(Duration::seconds(30));
        assert_eq!(clock.now().unix_timestamp(), 1_030);
        clock.advance(Duration::seconds(-10));
        assert_eq!(other.now().unix_timestamp(), 1_020);
    }

    #[test]
    fn system_clock_returns_a_recent_time() {
        assert!(SystemClock.now().unix_timestamp() > 1_700_000_000);
    }
}
