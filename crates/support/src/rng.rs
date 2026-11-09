//! Randomness abstraction.

use std::fmt::Debug;
use std::sync::{Mutex, PoisonError};

/// Failure of the operating system's random source (not recoverable by retrying).
#[derive(Debug, thiserror::Error)]
#[error("the operating system random source failed")]
pub struct RngError;

/// A source of random bytes. Tokens, codes and identifiers come only from here.
pub trait Rng: Send + Sync + Debug {
    /// Fill the buffer with random bytes.
    ///
    /// # Errors
    /// Returns [`RngError`] if the underlying source fails.
    fn fill(&self, buf: &mut [u8]) -> Result<(), RngError>;
}

/// The operating system's cryptographically secure random source.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRng;

impl Rng for SystemRng {
    fn fill(&self, buf: &mut [u8]) -> Result<(), RngError> {
        getrandom::fill(buf).map_err(|_| RngError)
    }
}

/// A deterministic generator for tests (SplitMix64). Never use outside tests.
#[derive(Debug)]
pub struct FakeRng {
    state: Mutex<u64>,
}

impl FakeRng {
    /// A generator with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: Mutex::new(seed),
        }
    }
}

impl Rng for FakeRng {
    fn fill(&self, buf: &mut [u8]) -> Result<(), RngError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        for chunk in buf.chunks_mut(8) {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = *state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let bytes = z.to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

/// Random bytes as a fixed-size array.
///
/// # Errors
/// Returns [`RngError`] if the underlying source fails.
pub fn random_array<const N: usize>(rng: &dyn Rng) -> Result<[u8; N], RngError> {
    let mut out = [0_u8; N];
    rng.fill(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_rng_is_deterministic_and_seed_dependent() {
        let a = random_array::<16>(&FakeRng::new(7)).unwrap();
        let b = random_array::<16>(&FakeRng::new(7)).unwrap();
        let c = random_array::<16>(&FakeRng::new(8)).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn fake_rng_advances_between_calls() {
        let rng = FakeRng::new(1);
        let first = random_array::<8>(&rng).unwrap();
        let second = random_array::<8>(&rng).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn system_rng_fills_buffers() {
        let a = random_array::<32>(&SystemRng).unwrap();
        let b = random_array::<32>(&SystemRng).unwrap();
        assert_ne!(a, b);
    }
}
