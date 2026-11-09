use super::*;

#[test]
fn secrets_can_come_from_files_and_lose_the_trailing_newline() {
    let mut fixture = Fixture::new();
    fixture.remove("TANDEM_DATABASE_URL");
    let path = fixture.write(
        "db.url",
        "postgres://app:p4ss@db/tandem?sslmode=verify-full\n",
    );
    fixture.set("TANDEM_DATABASE_URL_FILE", &path);
    let config = fixture.load().unwrap();
    assert_eq!(
        config.database.url.expose(),
        "postgres://app:p4ss@db/tandem?sslmode=verify-full"
    );
}

#[test]
fn a_secret_set_both_ways_or_from_an_empty_or_missing_file_is_refused() {
    let mut fixture = Fixture::new();
    let path = fixture.write("db.url", "postgres://x@db/t?sslmode=verify-full");
    fixture.set("TANDEM_DATABASE_URL_FILE", &path);
    assert!(fixture.problems()[0].1.contains("use one"));

    fixture.remove("TANDEM_DATABASE_URL");
    let empty = fixture.write("empty", "  \n");
    fixture.set("TANDEM_DATABASE_URL_FILE", &empty);
    assert!(fixture.problems()[0].1.contains("is empty"));

    fixture.set("TANDEM_DATABASE_URL_FILE", "/nonexistent/db.url");
    assert!(fixture.problems()[0].1.contains("cannot read"));
}

#[test]
fn the_file_supplies_values_and_the_environment_wins() {
    let mut fixture = Fixture::new();
    let file = fixture.write(
        "tandem.toml",
        "log_level = \"warn\"\nlisten_addr = \"0.0.0.0:9100\"\ntrusted_proxies = [\"10.0.0.0/8\", \"::1\"]\ndb_max_connections = 25\n",
    );
    fixture.set("TANDEM_CONFIG", &file);
    fixture.set("TANDEM_LOG_LEVEL", "error");
    let config = fixture.load().unwrap();
    assert_eq!(config.log_level, "error", "environment wins");
    assert_eq!(config.listen_addr.port(), 9100, "file beats the default");
    assert_eq!(config.database.max_connections, 25);
    assert!(config.trusted_proxies.contains("10.1.1.1".parse().unwrap()));
}

#[test]
fn a_secret_in_the_file_is_rejected_without_echoing_it() {
    let mut fixture = Fixture::new();
    let file = fixture.write(
        "tandem.toml",
        "metrics_token = \"super-secret-value-123\"\nmaster_key = \"k1 current AAAA\"\ndatabase_url = \"postgres://u:pw@h/d\"\n",
    );
    fixture.set("TANDEM_CONFIG", &file);
    let problems = fixture.problems();
    assert_eq!(problems.len(), 3);
    for (_, message) in &problems {
        assert!(message.contains("not the configuration file"));
        assert!(!message.contains("super-secret"));
    }
}

#[test]
fn unknown_keys_and_wrong_types_are_problems() {
    let mut fixture = Fixture::new();
    let file = fixture.write("tandem.toml", "colour = \"blue\"\nlog_level = 3.5\n");
    fixture.set("TANDEM_CONFIG", &file);
    let problems = fixture.problems();
    assert_eq!(problems.len(), 2);
    assert!(problems.iter().any(|p| p.1.contains("not a known setting")));
    assert!(problems.iter().any(|p| p.1.contains("wrong type")));
}

#[test]
fn an_unreadable_or_malformed_file_is_one_problem_that_quotes_nothing() {
    let mut fixture = Fixture::new();
    fixture.set("TANDEM_CONFIG", "/nonexistent/tandem.toml");
    assert!(fixture.problems()[0].1.contains("cannot read"));

    let file = fixture.write("bad.toml", "metrics_token = \"leaky-secret-9999\n");
    fixture.set("TANDEM_CONFIG", &file);
    let problems = fixture.problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].1.contains("not valid TOML"));
    assert!(!problems[0].1.contains("leaky"));
}
