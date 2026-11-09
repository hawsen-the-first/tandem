use tandem_credentials::BloomFilter;

#[test]
fn br_2_3_the_embedded_filter_knows_common_breached_passwords_and_not_a_fresh_passphrase() {
    let filter = BloomFilter::embedded().unwrap();
    for common in ["password123456", "qwertyuiop", "1q2w3e4r5t6y", "administrator"] {
        assert!(filter.contains_password(common), "{common} should be listed");
    }
    assert!(!filter.contains_password("Tr0ub4dor&3-unlisted-9x"));
    assert!(!filter.contains_password("correct horse battery staple 7"));
}

#[test]
fn the_embedded_filter_is_the_real_list_not_the_placeholder() {
    let filter = BloomFilter::embedded().unwrap();
    assert!(filter.byte_len() > 100_000, "the placeholder is a few dozen bytes");
}
