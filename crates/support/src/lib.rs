//! Clock and random-source traits, circuit breaker, backoff and supervised tasks.
//!
//! Everything that needs the time or randomness takes a [`Clock`] or [`Rng`] so tests can control
//! both without sleeping.

pub mod backoff;
pub mod breaker;
pub mod clock;
pub mod rng;
pub mod supervise;

pub use backoff::Backoff;
pub use breaker::{BreakerConfig, BreakerState, CallError, CircuitBreaker};
pub use clock::{Clock, FakeClock, SystemClock};
pub use rng::{FakeRng, Rng, RngError, SystemRng, random_array};
pub use supervise::supervise;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-support";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
