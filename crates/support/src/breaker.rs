//! Circuit breaker for outbound calls (RESILIENCY-10).
//!
//! Opens after `consecutive_failures` failures in a row, or when at least `failure_ratio` of the
//! last `window` calls failed. Stays open for `open_for`, then lets one trial call through.

use crate::clock::Clock;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use time::{Duration, OffsetDateTime};

/// Breaker thresholds.
#[derive(Debug, Clone, Copy)]
pub struct BreakerConfig {
    /// Failures in a row that open the breaker.
    pub consecutive_failures: u32,
    /// Number of recent calls considered for the failure ratio.
    pub window: usize,
    /// Fraction of failures in a full window that opens the breaker (0.5 = half).
    pub failure_ratio: f64,
    /// How long the breaker stays open before allowing a trial call.
    pub open_for: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            consecutive_failures: 5,
            window: 20,
            failure_ratio: 0.5,
            open_for: Duration::seconds(30),
        }
    }
}

/// Observable breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Calls flow normally.
    Closed,
    /// Calls are refused until the open period ends.
    Open,
    /// One trial call is in flight; others are refused.
    HalfOpen,
}

/// Outcome of [`CircuitBreaker::call`].
#[derive(Debug, thiserror::Error)]
pub enum CallError<E> {
    /// The breaker refused the call.
    #[error("circuit breaker is open")]
    Open,
    /// The call ran and failed.
    #[error(transparent)]
    Inner(E),
}

#[derive(Debug)]
enum Mode {
    Closed,
    Open { until: OffsetDateTime },
    HalfOpen,
}

#[derive(Debug)]
struct Inner {
    mode: Mode,
    consecutive_failures: u32,
    recent: VecDeque<bool>,
}

/// A thread-safe circuit breaker.
#[derive(Debug)]
pub struct CircuitBreaker {
    name: &'static str,
    config: BreakerConfig,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

impl CircuitBreaker {
    /// A closed breaker.
    pub fn new(name: &'static str, config: BreakerConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            name,
            config,
            clock,
            inner: Mutex::new(Inner {
                mode: Mode::Closed,
                consecutive_failures: 0,
                recent: VecDeque::new(),
            }),
        }
    }

    /// The breaker's name, used in metrics and logs.
    pub fn name(&self) -> &'static str {
        self.name
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Current state (an open breaker whose period has ended still reports `Open` until the next
    /// call is attempted).
    pub fn state(&self) -> BreakerState {
        match self.lock().mode {
            Mode::Closed => BreakerState::Closed,
            Mode::Open { .. } => BreakerState::Open,
            Mode::HalfOpen => BreakerState::HalfOpen,
        }
    }

    /// Ask permission to make a call. Pair every `true` with [`Self::on_success`] or
    /// [`Self::on_failure`].
    pub fn allow(&self) -> bool {
        let now = self.clock.now();
        let mut inner = self.lock();
        match inner.mode {
            Mode::Closed => true,
            Mode::HalfOpen => false,
            Mode::Open { until } => {
                if now >= until {
                    inner.mode = Mode::HalfOpen;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful call.
    pub fn on_success(&self) {
        let mut inner = self.lock();
        match inner.mode {
            Mode::HalfOpen => {
                inner.mode = Mode::Closed;
                inner.consecutive_failures = 0;
                inner.recent.clear();
            }
            Mode::Closed => {
                inner.consecutive_failures = 0;
                self.push_result(&mut inner, true);
            }
            Mode::Open { .. } => {}
        }
    }

    /// Record a failed call.
    pub fn on_failure(&self) {
        let now = self.clock.now();
        let mut inner = self.lock();
        match inner.mode {
            Mode::HalfOpen => {
                inner.mode = Mode::Open {
                    until: now + self.config.open_for,
                };
            }
            Mode::Closed => {
                inner.consecutive_failures += 1;
                self.push_result(&mut inner, false);
                if self.should_trip(&inner) {
                    inner.mode = Mode::Open {
                        until: now + self.config.open_for,
                    };
                    inner.consecutive_failures = 0;
                    inner.recent.clear();
                }
            }
            Mode::Open { .. } => {}
        }
    }

    fn push_result(&self, inner: &mut Inner, ok: bool) {
        inner.recent.push_back(ok);
        while inner.recent.len() > self.config.window {
            inner.recent.pop_front();
        }
    }

    fn should_trip(&self, inner: &Inner) -> bool {
        if inner.consecutive_failures >= self.config.consecutive_failures {
            return true;
        }
        if inner.recent.len() < self.config.window {
            return false;
        }
        let failures = inner.recent.iter().filter(|ok| !**ok).count();
        #[allow(clippy::cast_precision_loss)]
        let ratio = failures as f64 / inner.recent.len() as f64;
        ratio >= self.config.failure_ratio
    }

    /// Run `operation` under the breaker.
    ///
    /// # Errors
    /// [`CallError::Open`] when refused; [`CallError::Inner`] when the operation fails.
    pub async fn call<T, E, F>(&self, operation: F) -> Result<T, CallError<E>>
    where
        F: Future<Output = Result<T, E>>,
    {
        if !self.allow() {
            return Err(CallError::Open);
        }
        match operation.await {
            Ok(value) => {
                self.on_success();
                Ok(value)
            }
            Err(error) => {
                self.on_failure();
                Err(CallError::Inner(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FakeClock;

    fn breaker(clock: &FakeClock) -> CircuitBreaker {
        CircuitBreaker::new("test", BreakerConfig::default(), Arc::new(clock.clone()))
    }

    #[test]
    fn opens_after_five_consecutive_failures() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        for _ in 0..4 {
            assert!(b.allow());
            b.on_failure();
        }
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allow());
        b.on_failure();
        assert_eq!(b.state(), BreakerState::Open);
        assert!(!b.allow());
    }

    #[test]
    fn opens_when_half_of_a_full_window_failed_without_a_long_streak() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        // Alternate so there is never a streak of five, ending with 10 failures out of 20.
        for _ in 0..10 {
            b.on_success();
            b.on_failure();
        }
        assert_eq!(b.state(), BreakerState::Open);
    }

    #[test]
    fn a_success_resets_the_consecutive_count() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        for _ in 0..4 {
            b.on_failure();
        }
        b.on_success();
        for _ in 0..4 {
            b.on_failure();
        }
        assert_eq!(b.state(), BreakerState::Closed);
    }

    #[test]
    fn allows_one_trial_after_the_open_period_and_closes_on_success() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        for _ in 0..5 {
            b.on_failure();
        }
        assert!(!b.allow());
        clock.advance(Duration::seconds(29));
        assert!(!b.allow());
        clock.advance(Duration::seconds(1));
        assert!(b.allow());
        assert_eq!(b.state(), BreakerState::HalfOpen);
        assert!(!b.allow(), "only one trial call at a time");
        b.on_success();
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allow());
    }

    #[test]
    fn a_failed_trial_reopens_for_the_full_period() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        for _ in 0..5 {
            b.on_failure();
        }
        clock.advance(Duration::seconds(30));
        assert!(b.allow());
        b.on_failure();
        assert_eq!(b.state(), BreakerState::Open);
        clock.advance(Duration::seconds(29));
        assert!(!b.allow());
    }

    #[tokio::test]
    async fn call_wraps_an_operation() {
        let clock = FakeClock::at_unix(0);
        let b = breaker(&clock);
        let ok: Result<u8, CallError<()>> = b.call(async { Ok(1) }).await;
        assert!(matches!(ok, Ok(1)));
        for _ in 0..5 {
            let failed: Result<u8, CallError<&str>> = b.call(async { Err("boom") }).await;
            assert!(matches!(failed, Err(CallError::Inner("boom"))));
        }
        let refused: Result<u8, CallError<&str>> = b.call(async { Ok(2) }).await;
        assert!(matches!(refused, Err(CallError::Open)));
    }
}
