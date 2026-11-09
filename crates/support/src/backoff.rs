//! Exponential backoff.

use time::Duration;

/// Exponential backoff: `initial`, doubling each attempt, capped at `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
}

impl Backoff {
    /// A backoff starting at `initial` and never exceeding `max`.
    pub const fn new(initial: Duration, max: Duration) -> Self {
        Self { initial, max }
    }

    /// Delay before the next try after `attempt` failures so far (0 gives `initial`).
    pub fn delay(&self, attempt: u32) -> Duration {
        let factor = 1_i64.checked_shl(attempt.min(32)).unwrap_or(i64::MAX);
        let millis = self
            .initial
            .whole_milliseconds()
            .saturating_mul(i128::from(factor));
        let capped = millis.min(self.max.whole_milliseconds());
        Duration::milliseconds(i64::try_from(capped).unwrap_or(i64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_from_initial_and_caps_at_max() {
        let backoff = Backoff::new(Duration::seconds(1), Duration::seconds(30));
        let delays: Vec<i64> = (0..8).map(|n| backoff.delay(n).whole_seconds()).collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn huge_attempt_counts_do_not_overflow() {
        let backoff = Backoff::new(Duration::seconds(1), Duration::seconds(30));
        assert_eq!(backoff.delay(u32::MAX).whole_seconds(), 30);
    }
}
