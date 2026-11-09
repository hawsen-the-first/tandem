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
