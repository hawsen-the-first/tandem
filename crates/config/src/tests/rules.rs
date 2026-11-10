use super::*;

#[test]
fn a_minimal_environment_loads_with_defaults() {
    let config = Fixture::new().load().unwrap();
    assert_eq!(config.listen_addr, "127.0.0.1:8080".parse().unwrap());
    assert_eq!(config.log_level, "info");
    assert_eq!(config.hash_memory_budget_mb, 512);
    assert_eq!(config.database.max_connections, 10);
    assert!(config.metrics_token.is_none());
    assert!(config.otlp_endpoint.is_none());
    assert!(config.trusted_proxies.is_empty());
    assert_eq!(config.public_origin(), "https://tandem.example.test");
    assert_eq!(
        config.public_websocket_origin(),
        "wss://tandem.example.test"
    );
}

#[test]
fn every_required_setting_is_reported_when_missing() {
    let problems = Config::load(&BTreeMap::new()).unwrap_err().problems;
    let settings: Vec<_> = problems.iter().map(|p| p.setting.as_str()).collect();
    assert_eq!(
        settings,
        vec![
            "TANDEM_PUBLIC_URL",
            "TANDEM_DATABASE_URL",
            "TANDEM_DATABASE_CA_FILE",
            "TANDEM_MASTER_KEY_FILE"
        ]
    );
}

#[test]
fn http_is_accepted_only_for_loopback_hosts() {
    for url in [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://[::1]:5173",
    ] {
        let mut fixture = Fixture::new();
        fixture.set("TANDEM_PUBLIC_URL", url);
        assert!(fixture.load().is_ok(), "{url}");
    }
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_PUBLIC_URL", "http://tandem.example.test");
    let problems = fixture.problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].1.contains("https://"));
}

#[test]
fn the_public_url_must_be_an_origin() {
    for url in [
        "https://t.example.test/app",
        "https://t.example.test/?x=1",
        "https://user:pw@t.example.test",
        "ftp://t.example.test",
        "not a url",
    ] {
        let mut fixture = Fixture::new();
        fixture.set("TANDEM_PUBLIC_URL", url);
        assert_eq!(fixture.problems().len(), 1, "{url}");
    }
}

#[test]
fn the_websocket_origin_follows_the_scheme_and_port() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_PUBLIC_URL", "http://localhost:5173");
    let config = fixture.load().unwrap();
    assert_eq!(config.public_websocket_origin(), "ws://localhost:5173");
    assert_eq!(config.public_origin(), "http://localhost:5173");
}

#[test]
fn the_database_url_must_demand_verified_tls() {
    for url in [
        "postgres://app:p4ss@db/tandem",
        "postgres://app:p4ss@db/tandem?sslmode=require",
        "postgres://app:p4ss@db/tandem?sslmode=disable",
    ] {
        let mut fixture = Fixture::new();
        fixture.set("TANDEM_DATABASE_URL", url);
        let problems = fixture.problems();
        assert_eq!(problems.len(), 1, "{url}");
        assert!(problems[0].1.contains("sslmode=verify-full"));
        assert!(!problems[0].1.contains("p4ss"), "no secret in the message");
    }
    let mut fixture = Fixture::new();
    fixture.set(
        "TANDEM_DATABASE_URL",
        "postgres://app:p4ss@db/tandem?sslmode=verify-ca",
    );
    assert!(fixture.load().is_ok());
}

#[test]
fn a_database_url_of_the_wrong_kind_is_rejected() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_DATABASE_URL", "mysql://app:p4ss@db/tandem");
    assert!(fixture.problems()[0].1.contains("postgres://"));
}

#[test]
fn missing_files_are_named_but_not_read() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_DATABASE_CA_FILE", "/nonexistent/ca.crt");
    fixture.set("TANDEM_MASTER_KEY_FILE", "/nonexistent/master.key");
    let problems = fixture.problems();
    assert_eq!(problems.len(), 2);
    assert!(problems.iter().all(|p| p.1.contains("not a readable file")));
}

#[test]
fn the_master_key_may_be_inline_but_not_both_ways() {
    let mut fixture = Fixture::new();
    fixture.remove("TANDEM_MASTER_KEY_FILE");
    fixture.set("TANDEM_MASTER_KEY", "k1 current AAAA");
    assert!(matches!(
        fixture.load().unwrap().master_key,
        MasterKeySource::Inline(_)
    ));
    let key_path = fixture.write("other.key", "k2 current BBBB");
    fixture.set("TANDEM_MASTER_KEY_FILE", &key_path);
    assert!(fixture.problems()[0].1.contains("use one"));
}

#[test]
fn trusted_proxies_parse_addresses_and_ranges() {
    let mut fixture = Fixture::new();
    fixture.set(
        "TANDEM_TRUSTED_PROXIES",
        "10.0.0.0/8, 192.168.1.5, ::1, fd00::/8",
    );
    let proxies = fixture.load().unwrap().trusted_proxies;
    assert!(proxies.contains("10.9.8.7".parse().unwrap()));
    assert!(proxies.contains("192.168.1.5".parse().unwrap()));
    assert!(!proxies.contains("192.168.1.6".parse().unwrap()));
    assert!(proxies.contains("::1".parse().unwrap()));
    assert!(proxies.contains("fd12::1".parse().unwrap()));
    assert!(!proxies.contains("11.0.0.1".parse().unwrap()));
}

#[test]
fn each_bad_trusted_proxy_entry_is_its_own_problem() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_TRUSTED_PROXIES", "10.0.0.0/33, banana, 1.2.3.4");
    let problems = fixture.problems();
    assert_eq!(problems.len(), 2);
    assert!(problems[0].1.contains("10.0.0.0/33"));
    assert!(problems[1].1.contains("banana"));
}

#[test]
fn a_zero_length_prefix_trusts_the_whole_family_and_families_do_not_mix() {
    let range = Cidr::parse("0.0.0.0/0").unwrap();
    assert!(range.contains("203.0.113.1".parse().unwrap()));
    assert!(!range.contains("::1".parse().unwrap()));
}

#[test]
fn numbers_are_range_checked_and_listen_addr_validated() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_HASH_MEMORY_BUDGET_MB", "8");
    fixture.set("TANDEM_DB_MAX_CONNECTIONS", "many");
    fixture.set("TANDEM_LISTEN_ADDR", "everywhere");
    fixture.set("TANDEM_LOG_LEVEL", "shout");
    fixture.set("TANDEM_OTLP_ENDPOINT", "collector");
    let settings: Vec<_> = fixture.problems().into_iter().map(|p| p.0).collect();
    assert_eq!(
        settings,
        vec![
            "TANDEM_LISTEN_ADDR",
            "TANDEM_OTLP_ENDPOINT",
            "TANDEM_LOG_LEVEL",
            "TANDEM_HASH_MEMORY_BUDGET_MB",
            "TANDEM_DB_MAX_CONNECTIONS"
        ]
    );
}

#[test]
fn optional_settings_are_applied_when_valid() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_LISTEN_ADDR", "0.0.0.0:9000");
    fixture.set("TANDEM_LOG_LEVEL", "DEBUG");
    fixture.set("TANDEM_OTLP_ENDPOINT", "http://collector:4317");
    fixture.set("TANDEM_HASH_MEMORY_BUDGET_MB", "1024");
    let config = fixture.load().unwrap();
    assert_eq!(config.listen_addr.port(), 9000);
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.otlp_endpoint.unwrap().host_str(), Some("collector"));
    assert_eq!(config.hash_memory_budget_mb, 1024);
}

#[test]
fn a_short_metrics_token_is_refused_and_never_echoed() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_METRICS_TOKEN", "short");
    let problems = fixture.problems();
    assert_eq!(problems.len(), 1);
    assert!(!problems[0].1.contains("short"));
    fixture.set("TANDEM_METRICS_TOKEN", "a-long-enough-token-value");
    assert!(fixture.load().unwrap().metrics_token.is_some());
}

#[test]
fn secrets_never_print() {
    let config = Fixture::new().load().unwrap();
    let printed = format!("{config:?}");
    assert!(!printed.contains("p4ss"));
    assert!(printed.contains("redacted"));
}

#[test]
fn the_error_lists_every_problem_one_per_line() {
    let error = Config::load(&BTreeMap::new()).unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("invalid configuration (4 problems)"));
    assert_eq!(text.lines().count(), 5);
}

#[test]
fn the_worker_listen_address_defaults_and_is_validated() {
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture.load().unwrap().worker_listen_addr,
        "127.0.0.1:8081".parse().unwrap()
    );
    fixture.set("TANDEM_WORKER_LISTEN_ADDR", "0.0.0.0:9101");
    assert_eq!(fixture.load().unwrap().worker_listen_addr.port(), 9101);
    fixture.set("TANDEM_WORKER_LISTEN_ADDR", "nowhere");
    assert_eq!(fixture.problems()[0].0, "TANDEM_WORKER_LISTEN_ADDR");
}
