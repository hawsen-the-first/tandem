//! Sign-in protection state (BR-3.1 to BR-3.3): progressive delay, then temporary lockout.

use time::{Duration, OffsetDateTime};

/// The failure that locks an account (the 10th within the window).
pub const LOCKOUT_THRESHOLD: u32 = 10;
/// Failures are counted over this rolling window.
pub const FAILURE_WINDOW: Duration = Duration::hours(1);
/// How long a lockout lasts (assumption A10).
pub const LOCKOUT_DURATION: Duration = Duration::minutes(15);
/// The longest progressive delay, in seconds.
pub const MAX_DELAY_SECONDS: i64 = 60;

/// Delay imposed after the `n`-th failure in the window: 1 s, 2 s, 4 s ... capped at 60 s.
pub fn delay_after_failures(n: u32) -> Duration {
    if n == 0 {
        return Duration::ZERO;
    }
    let exponent = (n - 1).min(10);
    Duration::seconds((1_i64 << exponent).min(MAX_DELAY_SECONDS))
}

/// What the guard says about a sign-in attempt made now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardDecision {
    /// The attempt may be checked.
    Proceed,
    /// Too soon after the last failure; try again at the given time.
    WaitUntil(OffsetDateTime),
    /// The account is locked until the given time.
    LockedUntil(OffsetDateTime),
}

/// What recording a failure caused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureOutcome {
    /// A delay now applies until the given time.
    Delayed(OffsetDateTime),
    /// The account was just locked until the given time.
    Locked(OffsetDateTime),
}

/// Failure counters for one account (or one unknown email, BR-3.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuardState {
    /// Failures in the current window.
    pub failure_count: u32,
    /// When the current window started.
    pub window_started_at: Option<OffsetDateTime>,
    /// No attempt is accepted before this time.
    pub delay_until: Option<OffsetDateTime>,
    /// The account is locked until this time.
    pub locked_until: Option<OffsetDateTime>,
}

impl GuardState {
    /// Decide whether an attempt at `now` may proceed.
    pub fn decision(&self, now: OffsetDateTime) -> GuardDecision {
        if let Some(until) = self.locked_until.filter(|until| *until > now) {
            return GuardDecision::LockedUntil(until);
        }
        if let Some(until) = self.delay_until.filter(|until| *until > now) {
            return GuardDecision::WaitUntil(until);
        }
        GuardDecision::Proceed
    }

    /// Record a failed attempt at `now`.
    pub fn record_failure(&mut self, now: OffsetDateTime) -> FailureOutcome {
        let window_expired = self
            .window_started_at
            .is_none_or(|started| now - started >= FAILURE_WINDOW);
        if window_expired {
            self.failure_count = 0;
            self.window_started_at = Some(now);
            self.locked_until = None;
        }
        self.failure_count += 1;
        if self.failure_count >= LOCKOUT_THRESHOLD {
            let until = now + LOCKOUT_DURATION;
            self.locked_until = Some(until);
            self.delay_until = None;
            FailureOutcome::Locked(until)
        } else {
            let until = now + delay_after_failures(self.failure_count);
            self.delay_until = Some(until);
            FailureOutcome::Delayed(until)
        }
    }

    /// Record a successful sign-in: all counters reset.
    pub fn record_success(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + seconds).unwrap()
    }

    #[test]
    fn br_3_1_delay_doubles_and_caps_at_60s() {
        let delays: Vec<i64> = (1..=9)
            .map(|n| delay_after_failures(n).whole_seconds())
            .collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(delay_after_failures(0), Duration::ZERO);
        assert_eq!(delay_after_failures(u32::MAX).whole_seconds(), 60);
    }

    #[test]
    fn br_3_1_attempts_before_the_delay_ends_are_refused() {
        let mut guard = GuardState::default();
        assert_eq!(guard.decision(t(0)), GuardDecision::Proceed);
        let FailureOutcome::Delayed(until) = guard.record_failure(t(0)) else {
            panic!()
        };
        assert_eq!(until, t(1));
        assert_eq!(guard.decision(t(0)), GuardDecision::WaitUntil(t(1)));
        assert_eq!(guard.decision(t(1)), GuardDecision::Proceed);
    }

    #[test]
    fn br_3_2_the_tenth_failure_in_an_hour_locks_for_fifteen_minutes() {
        let mut guard = GuardState::default();
        for i in 0..9 {
            assert!(matches!(
                guard.record_failure(t(i * 100)),
                FailureOutcome::Delayed(_)
            ));
        }
        let FailureOutcome::Locked(until) = guard.record_failure(t(900)) else {
            panic!()
        };
        assert_eq!(until, t(900 + 15 * 60));
        assert_eq!(guard.decision(t(901)), GuardDecision::LockedUntil(until));
        assert_eq!(guard.decision(until), GuardDecision::Proceed);
    }

    #[test]
    fn br_3_3_the_window_resets_after_an_hour_without_failures() {
        let mut guard = GuardState::default();
        for i in 0..5 {
            guard.record_failure(t(i));
        }
        assert_eq!(guard.failure_count, 5);
        guard.record_failure(t(3_700));
        assert_eq!(guard.failure_count, 1, "the old window was discarded");
    }

    #[test]
    fn br_3_3_success_resets_everything() {
        let mut guard = GuardState::default();
        for i in 0..4 {
            guard.record_failure(t(i));
        }
        guard.record_success();
        assert_eq!(guard, GuardState::default());
        assert_eq!(guard.decision(t(5)), GuardDecision::Proceed);
    }

    #[test]
    fn br_3_2_failures_during_a_lock_keep_it_locked() {
        let mut guard = GuardState::default();
        for i in 0..10 {
            guard.record_failure(t(i));
        }
        assert!(matches!(
            guard.record_failure(t(20)),
            FailureOutcome::Locked(_)
        ));
    }
}
