use super::*;
use std::sync::Mutex;
use tandem_support::{BreakerConfig, FakeClock};

fn filter_of(passwords: &[&str]) -> Arc<BloomFilter> {
    let mut builder = BloomFilterBuilder::with_capacity(100, 1e-6);
    for password in passwords {
        builder.insert_password(password);
    }
    Arc::new(builder.finish())
}

#[test]
fn br_2_3_listed_passwords_are_found_and_others_are_not() {
    let filter = filter_of(&["password", "123456", "letmein"]);
    assert!(filter.contains_password("password"));
    assert!(filter.contains_password("letmein"));
    assert!(!filter.contains_password("Tr0ub4dor&3-unlisted"));
}

#[test]
fn br_2_3_the_filter_round_trips_through_its_file_form() {
    let filter = filter_of(&["password"]);
    let restored = BloomFilter::from_bytes(&filter.to_bytes()).unwrap();
    assert_eq!(restored, *filter);
    assert!(restored.contains_password("password"));
}

#[test]
fn br_2_3_corrupt_or_foreign_filter_files_are_rejected() {
    let good = filter_of(&["a"]).to_bytes();
    assert!(BloomFilter::from_bytes(&[]).is_err());
    assert!(BloomFilter::from_bytes(&good[..good.len() - 1]).is_err());
    let mut wrong_magic = good.clone();
    wrong_magic[0] = b'X';
    assert!(BloomFilter::from_bytes(&wrong_magic).is_err());
    let mut zero_hashes = good;
    zero_hashes[8..12].copy_from_slice(&0_u32.to_le_bytes());
    assert!(BloomFilter::from_bytes(&zero_hashes).is_err());
}

#[test]
fn br_2_3_the_false_positive_rate_matches_the_design() {
    let mut builder = BloomFilterBuilder::with_capacity(10_000, 1e-3);
    for i in 0..10_000 {
        builder.insert_password(&format!("listed-{i}"));
    }
    let filter = builder.finish();
    assert!(
        (0..10_000).all(|i| filter.contains_password(&format!("listed-{i}"))),
        "no false negatives"
    );
    let false_positives = (0..100_000)
        .filter(|i| filter.contains_password(&format!("other-{i}")))
        .count();
    assert!(
        false_positives < 300,
        "expected about 100 false positives in 100,000, got {false_positives}"
    );
}

#[test]
fn corpus_lines_with_counts_are_accepted_and_bad_lines_rejected() {
    let mut builder = BloomFilterBuilder::with_capacity(10, 1e-6);
    builder
        .insert_hash_line("5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8:3730471")
        .unwrap();
    builder
        .insert_hash_line("5baa61e4c9b93f3f0682250b6cf8331b7ee68fd8")
        .unwrap();
    assert_eq!(
        builder.insert_hash_line("not hex"),
        Err(BloomFilterError::BadHashLine)
    );
    assert_eq!(
        builder.insert_hash_line("ABCDEF"),
        Err(BloomFilterError::BadHashLine)
    );
    assert!(
        builder.finish().contains_password("password"),
        "that hash is SHA-1 of `password`"
    );
}

#[test]
fn the_embedded_filter_parses() {
    assert!(BloomFilter::embedded().is_ok());
}

struct FakeFetcher {
    body: Mutex<Result<String, FetchError>>,
    requested: Mutex<Vec<String>>,
}

impl FakeFetcher {
    fn new(body: Result<String, FetchError>) -> Arc<Self> {
        Arc::new(Self {
            body: Mutex::new(body),
            requested: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl RangeFetcher for FakeFetcher {
    async fn fetch_range(&self, prefix: &str) -> Result<String, FetchError> {
        self.requested.lock().unwrap().push(prefix.to_owned());
        self.body.lock().unwrap().clone()
    }
}

fn online(fetcher: Arc<FakeFetcher>) -> OnlineBreachCheck {
    let breaker = CircuitBreaker::new(
        "breach",
        BreakerConfig::default(),
        Arc::new(FakeClock::at_unix(0)),
    );
    OnlineBreachCheck::new(fetcher, breaker, Duration::from_secs(2))
}

#[tokio::test]
async fn br_2_3_the_online_check_is_used_only_when_asked_and_sends_only_a_five_character_prefix() {
    let hash = sha1_hex_upper("Tr0ub4dor&3-online");
    let body = format!(
        "0000000000000000000000000000000000A:0\n{}:42\nFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:7",
        &hash[5..]
    );
    let fetcher = FakeFetcher::new(Ok(body));
    let checker = BreachCheckerImpl::with_online(filter_of(&[]), online(fetcher.clone()));
    assert!(
        !checker.is_breached("Tr0ub4dor&3-online", false).await,
        "offline only"
    );
    assert!(fetcher.requested.lock().unwrap().is_empty());
    assert!(checker.is_breached("Tr0ub4dor&3-online", true).await);
    assert_eq!(
        *fetcher.requested.lock().unwrap(),
        vec![hash[..5].to_owned()]
    );
}

#[tokio::test]
async fn padding_entries_with_a_zero_count_never_mean_breached() {
    let hash = sha1_hex_upper("padded-only");
    let fetcher = FakeFetcher::new(Ok(format!("{}:0", &hash[5..])));
    let checker = BreachCheckerImpl::with_online(filter_of(&[]), online(fetcher));
    assert!(!checker.is_breached("padded-only", true).await);
}

#[tokio::test]
async fn rl_2_when_the_online_service_fails_the_offline_result_stands_and_the_breaker_opens() {
    let fetcher = FakeFetcher::new(Err(FetchError("boom".into())));
    let checker = BreachCheckerImpl::with_online(filter_of(&["password"]), online(fetcher.clone()));
    for _ in 0..5 {
        assert!(!checker.is_breached("fresh-password-1", true).await);
    }
    assert_eq!(checker.degraded_total(), 5);
    assert_eq!(fetcher.requested.lock().unwrap().len(), 5);
    assert!(!checker.is_breached("fresh-password-1", true).await);
    assert_eq!(
        fetcher.requested.lock().unwrap().len(),
        5,
        "the open breaker made no sixth request"
    );
    assert_eq!(checker.degraded_total(), 6);
    assert!(
        checker.is_breached("password", true).await,
        "the offline list still decides"
    );
}

#[tokio::test(start_paused = true)]
async fn rl_1_a_hanging_online_service_is_cut_off_after_two_seconds() {
    struct Hanging;
    #[async_trait]
    impl RangeFetcher for Hanging {
        async fn fetch_range(&self, _prefix: &str) -> Result<String, FetchError> {
            std::future::pending().await
        }
    }
    let breaker = CircuitBreaker::new(
        "breach",
        BreakerConfig::default(),
        Arc::new(FakeClock::at_unix(0)),
    );
    let checker = BreachCheckerImpl::with_online(
        filter_of(&[]),
        OnlineBreachCheck::new(Arc::new(Hanging), breaker, Duration::from_secs(2)),
    );
    let started = tokio::time::Instant::now();
    assert!(!checker.is_breached("anything-1A!", true).await);
    assert_eq!(started.elapsed(), Duration::from_secs(2));
    assert_eq!(checker.degraded_total(), 1);
}
