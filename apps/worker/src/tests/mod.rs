use crate::{WorkerOptions, constant_time_eq};
use tandem_config::Config;

#[test]
fn constant_time_eq_matches_only_identical_bytes() {
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(constant_time_eq(b"", b""));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"abcd"));
    assert!(!constant_time_eq(b"abcd", b"abc"));
    assert!(!constant_time_eq(b"", b"a"));
}

#[test]
fn production_options_follow_the_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca");
    std::fs::write(&ca, "ca").unwrap();
    let environment = [
        (
            "TANDEM_PUBLIC_URL",
            "https://tandem.example.test".to_owned(),
        ),
        (
            "TANDEM_DATABASE_URL",
            "postgres://app:x@db/t?sslmode=verify-full".to_owned(),
        ),
        ("TANDEM_DATABASE_CA_FILE", ca.display().to_string()),
        ("TANDEM_MASTER_KEY_FILE", ca.display().to_string()),
        ("TANDEM_WORKER_LISTEN_ADDR", "127.0.0.1:9199".to_owned()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    let config = Config::load(&environment).unwrap();
    let options = WorkerOptions::from_config(&config);
    assert_eq!(options.listen_addr.port(), 9199);
    assert_eq!(options.cleanup_interval.as_secs(), 86_400);
    assert_eq!(options.drain_timeout.as_secs(), 30);
}
