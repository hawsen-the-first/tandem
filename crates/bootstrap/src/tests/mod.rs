use crate::{Shutdown, format_hash_profile, parse_hash_profile};
use tandem_credentials::HashProfile;

#[test]
fn a_profile_round_trips_through_its_stored_form() {
    let profile = HashProfile {
        memory_kib: 65_536,
        iterations: 3,
        lanes: 1,
    };
    assert_eq!(
        parse_hash_profile(&format_hash_profile(&profile)),
        Some(profile)
    );
}

#[test]
fn malformed_or_weak_stored_profiles_are_ignored() {
    for text in [
        "",
        "banana",
        "19456:2",
        "19456:2:1:9",
        "1024:2:1",
        "19456:1:1",
        "19456:2:0",
        "19456:x:1",
    ] {
        assert_eq!(parse_hash_profile(text), None, "{text:?}");
    }
    assert_eq!(parse_hash_profile("19456:2:1"), Some(HashProfile::MINIMUM));
}

#[tokio::test]
async fn shutdown_reaches_every_receiver_once_and_stays_triggered() {
    let shutdown = Shutdown::new();
    let mut first = shutdown.subscribe();
    let mut second = shutdown.subscribe();
    assert!(!shutdown.is_triggered());
    shutdown.trigger();
    shutdown.trigger();
    first.changed().await.unwrap();
    second.changed().await.unwrap();
    assert!(*first.borrow() && *second.borrow() && shutdown.is_triggered());
    assert!(
        *shutdown.clone().subscribe().borrow(),
        "late subscribers see it too"
    );
}
