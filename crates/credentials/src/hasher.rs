//! The Argon2id hasher pool: bounded concurrency, load shedding, transparent rehash (NFR design 4).

use crate::profile::HashProfile;
use argon2::{Argon2, Params};
use argon2::{PasswordHash, PasswordHasher as _, PasswordVerifier};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tandem_ports::{HashError, PasswordHasher, Verification};
use tandem_support::{Rng, random_array};
use tokio::sync::Semaphore;

const DUMMY_PASSWORD: &str = "tandem-dummy-password-for-timing";
const SALT_LEN: usize = 16;

/// Counters the telemetry layer exports (`hash_inflight`, `hash_rejected_total`).
#[derive(Debug, Default)]
pub struct HasherStats {
    /// Hashes running now.
    pub inflight: AtomicUsize,
    /// Attempts refused because no slot freed up in time.
    pub rejected_total: AtomicU64,
    /// Hashes completed.
    pub completed_total: AtomicU64,
}

/// Argon2id hashing behind a concurrency gate.
pub struct ArgonHasher {
    profile: HashProfile,
    gate: Arc<Semaphore>,
    max_wait: Duration,
    rng: Arc<dyn Rng>,
    dummy_hash: String,
    stats: Arc<HasherStats>,
}

impl std::fmt::Debug for ArgonHasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArgonHasher")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

/// How many hashes may run at once: the smaller of the CPU count and what the memory budget
/// allows, and at least one.
pub fn concurrency_limit(cpu_cores: usize, memory_budget_mib: u32, profile: &HashProfile) -> usize {
    let by_memory = usize::try_from(memory_budget_mib / profile.memory_mib().max(1)).unwrap_or(1);
    cpu_cores.min(by_memory).max(1)
}

fn hash_blocking(profile: HashProfile, rng: &dyn Rng, password: &str) -> Result<String, HashError> {
    let argon2 = profile
        .argon2()
        .map_err(|error| HashError::Internal(error.to_string()))?;
    let salt =
        random_array::<SALT_LEN>(rng).map_err(|error| HashError::Internal(error.to_string()))?;
    argon2
        .hash_password_with_salt(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| HashError::Internal(error.to_string()))
}

fn verify_blocking(profile: HashProfile, password: &str, stored: &str) -> Verification {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return Verification {
            matches: false,
            needs_rehash: false,
        };
    };
    let matches = Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();
    let needs_rehash = matches
        && Params::try_from(&parsed).map_or(true, |params| {
            params.m_cost() != profile.memory_kib
                || params.t_cost() != profile.iterations
                || params.p_cost() != profile.lanes
        });
    Verification {
        matches,
        needs_rehash,
    }
}

impl ArgonHasher {
    /// Create a hasher. `concurrency` is the number of simultaneous hashes; `max_wait` is how long
    /// a caller waits for a slot before being told the system is busy.
    ///
    /// # Errors
    /// [`HashError::Internal`] when the profile is invalid or the dummy hash cannot be computed.
    pub fn new(
        profile: HashProfile,
        concurrency: usize,
        max_wait: Duration,
        rng: Arc<dyn Rng>,
    ) -> Result<Self, HashError> {
        let dummy_hash = hash_blocking(profile, &*rng, DUMMY_PASSWORD)?;
        Ok(Self {
            profile,
            gate: Arc::new(Semaphore::new(concurrency.max(1))),
            max_wait,
            rng,
            dummy_hash,
            stats: Arc::new(HasherStats::default()),
        })
    }

    /// The profile new hashes use.
    pub fn profile(&self) -> HashProfile {
        self.profile
    }

    /// Counters for metrics.
    pub fn stats(&self) -> Arc<HasherStats> {
        Arc::clone(&self.stats)
    }

    async fn run<T, F>(&self, work: F) -> Result<T, HashError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit =
            match tokio::time::timeout(self.max_wait, Arc::clone(&self.gate).acquire_owned()).await
            {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => return Err(HashError::Internal("the hasher is shut down".into())),
                Err(_) => {
                    self.stats.rejected_total.fetch_add(1, Ordering::Relaxed);
                    return Err(HashError::Busy);
                }
            };
        self.stats.inflight.fetch_add(1, Ordering::Relaxed);
        let outcome = tokio::task::spawn_blocking(work).await;
        self.stats.inflight.fetch_sub(1, Ordering::Relaxed);
        drop(permit);
        let value = outcome
            .map_err(|error| HashError::Internal(format!("hashing task failed: {error}")))?;
        self.stats.completed_total.fetch_add(1, Ordering::Relaxed);
        Ok(value)
    }
}

#[async_trait]
impl PasswordHasher for ArgonHasher {
    async fn hash(&self, password: &str) -> Result<String, HashError> {
        let (profile, rng, password) = (self.profile, Arc::clone(&self.rng), password.to_owned());
        self.run(move || hash_blocking(profile, &*rng, &password))
            .await?
    }

    async fn verify(&self, password: &str, stored: &str) -> Result<Verification, HashError> {
        let (profile, password, stored) = (self.profile, password.to_owned(), stored.to_owned());
        self.run(move || verify_blocking(profile, &password, &stored))
            .await
    }

    async fn dummy_verify(&self) -> Result<(), HashError> {
        let (profile, stored) = (self.profile, self.dummy_hash.clone());
        self.run(move || {
            verify_blocking(profile, "a different password", &stored);
        })
        .await
    }
}

#[cfg(test)]
mod tests;
