use super::*;
use tandem_support::SystemRng;

/// RFC 6238 appendix B, SHA-1 secret "12345678901234567890". The RFC lists 8-digit values; the
/// 6-digit code is the last six digits of the same truncated value.
const SECRET: &[u8] = b"12345678901234567890";

fn totp() -> HmacTotp {
    HmacTotp::new(Arc::new(SystemRng))
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).unwrap()
}

#[test]
fn br_4_3_matches_the_rfc_6238_test_vectors() {
    for (time, code) in [
        (59_i64, "287082"),
        (1_111_111_109, "081804"),
        (1_111_111_111, "050471"),
        (1_234_567_890, "005924"),
        (2_000_000_000, "279037"),
        (20_000_000_000, "353130"),
    ] {
        let step = u64::try_from(time).unwrap() / 30;
        assert_eq!(
            HmacTotp::code_for_step(SECRET, step).as_deref(),
            Some(code),
            "t={time}"
        );
    }
}

#[test]
fn br_4_3_a_code_is_accepted_one_step_either_side_but_not_two() {
    let service = totp();
    let t = 1_234_567_890_i64;
    let code = HmacTotp::code_for_step(SECRET, u64::try_from(t).unwrap() / 30).unwrap();
    assert_eq!(
        service.verify(SECRET, &code, at(t)),
        Some(u64::try_from(t).unwrap() / 30)
    );
    assert!(
        service.verify(SECRET, &code, at(t + 30)).is_some(),
        "one step later"
    );
    assert!(
        service.verify(SECRET, &code, at(t - 30)).is_some(),
        "one step earlier"
    );
    assert!(
        service.verify(SECRET, &code, at(t + 90)).is_none(),
        "three steps later"
    );
    assert!(service.verify(SECRET, &code, at(t - 90)).is_none());
}

#[test]
fn br_4_4_the_matching_step_is_reported_so_replays_can_be_refused() {
    let service = totp();
    let t = 1_800_000_000_i64;
    let step = u64::try_from(t).unwrap() / 30;
    let previous = HmacTotp::code_for_step(SECRET, step - 1).unwrap();
    assert_eq!(service.verify(SECRET, &previous, at(t)), Some(step - 1));
}

#[test]
fn malformed_codes_are_rejected_without_computing_anything() {
    let service = totp();
    for bad in ["", "12345", "1234567", "12345a", " 12345", "١٢٣٤٥٦"] {
        assert!(service.verify(SECRET, bad, at(59)).is_none(), "{bad:?}");
    }
}

#[test]
fn secrets_are_20_random_bytes_and_base32_has_no_padding() {
    let service = totp();
    let a = service.generate_secret().unwrap();
    assert_eq!(a.len(), 20);
    assert_ne!(a, service.generate_secret().unwrap());
    let text = service.secret_base32(SECRET);
    assert_eq!(text, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    assert!(!text.contains('='));
}

#[test]
fn the_otpauth_uri_escapes_the_label_and_carries_the_parameters() {
    let uri = totp().otpauth_uri(SECRET, "ana+test@example.com", "Tandem Co");
    assert!(uri.starts_with(
        "otpauth://totp/Tandem%20Co:ana%2Btest%40example.com?secret=GEZDGNBVGY3TQOJQ"
    ));
    assert!(uri.contains("&issuer=Tandem%20Co&algorithm=SHA1&digits=6&period=30"));
}
