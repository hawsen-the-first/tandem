use super::*;

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

#[test]
fn br_19_1_accepts_ordinary_https_and_local_http_urls_and_normalises_them() {
    for (input, expected) in [
        ("https://api.openai.com/v1", "https://api.openai.com/v1"),
        ("  https://api.openai.com/v1  ", "https://api.openai.com/v1"),
        ("HTTPS://API.OpenAI.com/v1", "https://api.openai.com/v1"),
        (
            "https://openrouter.ai/api/v1/",
            "https://openrouter.ai/api/v1",
        ),
        ("https://example.test/", "https://example.test"),
        ("https://example.test:8443/x", "https://example.test:8443/x"),
        ("https://example.test:443/x", "https://example.test/x"),
        ("http://localhost:11434/v1", "http://localhost:11434/v1"),
        ("http://127.0.0.1:1234/v1", "http://127.0.0.1:1234/v1"),
        ("http://[::1]:8080", "http://[::1]:8080"),
        ("http://192.168.1.20:8000/v1", "http://192.168.1.20:8000/v1"),
        ("http://10.0.0.5", "http://10.0.0.5"),
        ("http://172.16.4.4:9000", "http://172.16.4.4:9000"),
        ("http://100.64.1.1:11434", "http://100.64.1.1:11434"),
        ("http://[fd12:3456::1]:80", "http://[fd12:3456::1]"),
        ("http://ollama:11434/v1", "http://ollama:11434/v1"),
        ("http://searxng:8080", "http://searxng:8080"),
        ("http://gpu-box.local:8000", "http://gpu-box.local:8000"),
        ("http://models.internal/v1", "http://models.internal/v1"),
        ("http://foo.localhost:3000", "http://foo.localhost:3000"),
        ("http://localhost.:11434", "http://localhost.:11434"),
        ("https://203.0.113.9/v1", "https://203.0.113.9/v1"),
        ("https://192.168.1.20", "https://192.168.1.20"),
    ] {
        let parsed = OutboundUrl::parse(input).unwrap_or_else(|error| panic!("{input}: {error}"));
        assert_eq!(parsed.as_str(), expected, "{input}");
    }
}

#[test]
fn br_19_1_refuses_malformed_and_unsafe_shapes() {
    for (input, refusal) in [
        ("", UrlRefusal::Empty),
        ("   ", UrlRefusal::Empty),
        ("not a url", UrlRefusal::Malformed),
        ("https://", UrlRefusal::Malformed),
        ("ftp://example.test/x", UrlRefusal::UnsupportedScheme),
        ("file:///etc/passwd", UrlRefusal::UnsupportedScheme),
        ("javascript:alert(1)", UrlRefusal::UnsupportedScheme),
        ("ollama:11434", UrlRefusal::UnsupportedScheme),
        ("https://user:pw@example.test/v1", UrlRefusal::Credentials),
        ("https://user@example.test/v1", UrlRefusal::Credentials),
        (
            "https://example.test/v1?key=abc",
            UrlRefusal::QueryOrFragment,
        ),
        ("https://example.test/v1#top", UrlRefusal::QueryOrFragment),
    ] {
        assert_eq!(OutboundUrl::parse(input).unwrap_err(), refusal, "{input}");
    }
    let long = format!("https://example.test/{}", "a".repeat(MAX_URL_LENGTH));
    assert_eq!(OutboundUrl::parse(&long).unwrap_err(), UrlRefusal::TooLong);
}

#[test]
fn br_19_2_http_is_only_for_local_and_private_hosts() {
    for input in [
        "http://example.test/v1",
        "http://api.openai.com",
        "http://8.8.8.8",
        "http://203.0.113.9:11434",
        "http://[2001:db8::1]/",
        "http://foo.localhost.evil.test",
        "http://local",
        "http://notlocal.test",
    ] {
        let result = OutboundUrl::parse(input);
        if input == "http://local" {
            assert!(result.is_ok(), "a single-label name is local: {input}");
        } else {
            assert_eq!(result.unwrap_err(), UrlRefusal::HttpToPublicHost, "{input}");
        }
    }
}

#[test]
fn br_19_3_link_local_and_metadata_addresses_are_refused_in_every_spelling_and_scheme() {
    for input in [
        "http://169.254.169.254/latest/meta-data",
        "https://169.254.169.254/",
        "http://169.254.1.1",
        "http://[fe80::1]/",
        "https://[fe80::abcd%25eth0]/",
        "http://[fd00:ec2::254]/",
        "https://[fd00:ec2::254]/",
        "http://[::ffff:169.254.169.254]/",
        "http://[::ffff:a9fe:a9fe]/",
        "http://[::169.254.169.254]/",
        "http://[64:ff9b::a9fe:a9fe]/",
        "http://0xa9.0xfe.0xa9.0xfe/",
        "http://0251.0376.0251.0376/",
        "http://2852039166/",
        "http://0xa9fea9fe/",
        "http://169.254.169.254.:80/",
        "http://100.100.100.200/",
        "https://168.63.129.16/",
        "http://192.0.0.192/",
    ] {
        assert!(
            OutboundUrl::parse(input).is_err(),
            "{input} must be refused"
        );
    }
    assert_eq!(
        OutboundUrl::parse("http://169.254.169.254/").unwrap_err(),
        UrlRefusal::BlockedAddress
    );
}

#[test]
fn br_19_3_unusable_addresses_are_refused() {
    for input in [
        "https://224.0.0.1/",
        "http://255.255.255.255/",
        "https://240.0.0.1/",
        "https://[ff02::1]/",
    ] {
        assert_eq!(
            OutboundUrl::parse(input).unwrap_err(),
            UrlRefusal::UnusableAddress,
            "{input}"
        );
    }
}

#[test]
fn classification_covers_the_ranges() {
    for (address, class) in [
        ("127.0.0.1", AddressClass::Loopback),
        ("127.255.255.254", AddressClass::Loopback),
        ("0.0.0.0", AddressClass::Loopback),
        ("::1", AddressClass::Loopback),
        ("::", AddressClass::Loopback),
        ("10.1.2.3", AddressClass::Private),
        ("172.16.0.1", AddressClass::Private),
        ("172.31.255.255", AddressClass::Private),
        ("172.32.0.1", AddressClass::Public),
        ("192.168.0.1", AddressClass::Private),
        ("100.64.0.1", AddressClass::Private),
        ("100.127.255.255", AddressClass::Private),
        ("100.128.0.1", AddressClass::Public),
        ("fc00::1", AddressClass::Private),
        ("fdff::1", AddressClass::Private),
        ("fe80::1", AddressClass::LinkLocal),
        ("febf::1", AddressClass::LinkLocal),
        ("fec0::1", AddressClass::Public),
        ("169.254.0.1", AddressClass::LinkLocal),
        ("169.254.169.254", AddressClass::Metadata),
        ("100.100.100.200", AddressClass::Metadata),
        ("fd00:ec2::254", AddressClass::Metadata),
        ("8.8.8.8", AddressClass::Public),
        ("2606:4700::1111", AddressClass::Public),
        ("224.0.0.251", AddressClass::Unusable),
        ("::ffff:10.0.0.1", AddressClass::Private),
        ("::ffff:8.8.8.8", AddressClass::Public),
        ("::ffff:127.0.0.1", AddressClass::Loopback),
        ("::ffff:169.254.169.254", AddressClass::Metadata),
        ("64:ff9b::808:808", AddressClass::Public),
        ("64:ff9b::a9fe:a9fe", AddressClass::Metadata),
    ] {
        assert_eq!(classify(ip(address)), class, "{address}");
    }
}

#[test]
fn only_link_local_metadata_and_unusable_addresses_are_always_blocked() {
    for blocked in [
        "169.254.1.1",
        "fe80::1",
        "169.254.169.254",
        "fd00:ec2::254",
        "224.0.0.1",
    ] {
        assert!(is_always_blocked(ip(blocked)), "{blocked}");
    }
    for allowed in [
        "127.0.0.1",
        "10.0.0.1",
        "192.168.1.1",
        "8.8.8.8",
        "::1",
        "fd12::1",
    ] {
        assert!(!is_always_blocked(ip(allowed)), "{allowed}");
    }
}

#[test]
fn accessors_describe_the_parsed_url() {
    let url = OutboundUrl::parse("http://192.168.1.20:8000/v1/").unwrap();
    assert_eq!(url.port(), 8000);
    assert!(!url.is_https());
    assert_eq!(url.literal_address(), Some(ip("192.168.1.20")));
    assert_eq!(url.host_text(), "192.168.1.20");
    assert_eq!(url.join("/models"), "http://192.168.1.20:8000/v1/models");
    let named = OutboundUrl::parse("https://api.example.test").unwrap();
    assert_eq!(named.port(), 443);
    assert!(named.is_https());
    assert_eq!(named.literal_address(), None);
    assert_eq!(named.host_text(), "api.example.test");
    assert_eq!(named.join("/x"), "https://api.example.test/x");
    assert_eq!(format!("{named}"), "https://api.example.test");
}
