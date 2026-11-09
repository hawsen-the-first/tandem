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
mod tests {
    use super::*;
    use tandem_support::SystemRng;

    /// Small and fast, only for tests; the real minimum is enforced when parsing stored profiles.
    const TEST_PROFILE: HashProfile = HashProfile {
        memory_kib: 64,
        iterations: 1,
        lanes: 1,
    };

    fn hasher(concurrency: usize, wait: Duration) -> ArgonHasher {
        ArgonHasher::new(TEST_PROFILE, concurrency, wait, Arc::new(SystemRng)).unwrap()
    }

    #[tokio::test]
    async fn se_13_hashes_verify_and_wrong_passwords_do_not() {
        let h = hasher(2, Duration::from_secs(2));
        let stored = h.hash("Correct-Horse-7").await.unwrap();
        assert!(stored.starts_with("$argon2id$v=19$m=64,t=1,p=1$"));
        assert!(h.verify("Correct-Horse-7", &stored).await.unwrap().matches);
        assert!(!h.verify("Wrong-Horse-7", &stored).await.unwrap().matches);
    }

    #[tokio::test]
    async fn every_hash_has_its_own_salt() {
        let h = hasher(2, Duration::from_secs(2));
        assert_ne!(h.hash("same").await.unwrap(), h.hash("same").await.unwrap());
    }

    #[tokio::test]
    async fn pf_4_a_hash_made_with_other_parameters_asks_for_a_rehash() {
        let old = hasher(1, Duration::from_secs(2));
        let stored = old.hash("Correct-Horse-7").await.unwrap();
        let newer = ArgonHasher::new(
            HashProfile {
                memory_kib: 128,
                iterations: 2,
                lanes: 1,
            },
            1,
            Duration::from_secs(2),
            Arc::new(SystemRng),
        )
        .unwrap();
        let verdict = newer.verify("Correct-Horse-7", &stored).await.unwrap();
        assert!(verdict.matches && verdict.needs_rehash);
        let same = old.verify("Correct-Horse-7", &stored).await.unwrap();
        assert!(same.matches && !same.needs_rehash);
    }

    #[tokio::test]
    async fn malformed_stored_hashes_simply_do_not_match() {
        let h = hasher(1, Duration::from_secs(2));
        for stored in [
            "",
            "plaintext",
            "$argon2id$garbage",
            "$2b$12$abcdefghijklmnopqrstuv",
        ] {
            let verdict = h.verify("anything", stored).await.unwrap();
            assert!(!verdict.matches && !verdict.needs_rehash, "{stored:?}");
        }
    }

    #[tokio::test]
    async fn br_3_5_the_dummy_verification_does_the_same_work_without_matching() {
        let h = hasher(1, Duration::from_secs(2));
        h.dummy_verify().await.unwrap();
        assert_eq!(h.stats().completed_total.load(Ordering::Relaxed), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn nfr_design_4_when_every_slot_is_busy_a_caller_waits_then_gets_busy() {
        let h = Arc::new(hasher(1, Duration::from_secs(2)));
        let held = Arc::clone(&h.gate).acquire_owned().await.unwrap();
        let started = tokio::time::Instant::now();
        let result = h.hash("Correct-Horse-7").await;
        assert_eq!(result, Err(HashError::Busy));
        assert_eq!(
            started.elapsed(),
            Duration::from_secs(2),
            "it waited the full two seconds"
        );
        assert_eq!(h.stats().rejected_total.load(Ordering::Relaxed), 1);
        drop(held);
        assert!(
            h.hash("Correct-Horse-7").await.is_ok(),
            "capacity returns once a slot frees up"
        );
    }

    #[test]
    fn nfr_design_4_concurrency_is_bounded_by_cpu_and_by_the_memory_budget() {
        let profile = HashProfile {
            memory_kib: 64 * 1024,
            iterations: 3,
            lanes: 1,
        };
        assert_eq!(concurrency_limit(8, 512, &profile), 8);
        assert_eq!(concurrency_limit(16, 512, &profile), 8, "512 MiB / 64 MiB");
        assert_eq!(concurrency_limit(8, 100, &profile), 1);
        assert_eq!(concurrency_limit(8, 10, &profile), 1, "never below one");
    }
}
