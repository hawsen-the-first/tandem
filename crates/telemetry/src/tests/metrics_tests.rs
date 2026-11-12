use crate::metrics::{MetricsRegistry, names};

#[test]
fn every_ob1_metric_is_exposed_from_the_start() {
    let registry = MetricsRegistry::new("1.2.3").unwrap();
    let text = registry.render();
    for name in [
        names::AUTH_SIGN_IN_TOTAL,
        names::AUTH_LOCKOUTS_TOTAL,
        names::AUTHZ_DENIED_TOTAL,
        names::DB_POOL_CONNECTIONS,
        names::DB_QUERY_DURATION_SECONDS,
        names::OUTBOX_PENDING,
        names::JOB_QUEUE_DEPTH,
        names::JOB_OLDEST_AGE_SECONDS,
        names::WS_CONNECTIONS,
        names::HASH_INFLIGHT,
        names::HASH_REJECTED_TOTAL,
        names::HASH_WAIT_SECONDS,
        names::BREACH_CHECK_DEGRADED_TOTAL,
        names::OUTBOX_RELAY_LAG_SECONDS,
        names::BUILD_INFO,
    ] {
        assert!(text.contains(name), "{name} missing from:\n{text}");
    }
    assert!(text.contains("build_info{version=\"1.2.3\"} 1"));
}

#[test]
fn recording_lands_in_the_registry_with_labels() {
    let registry = MetricsRegistry::new("0").unwrap();
    metrics::with_local_recorder(registry.recorder(), || {
        metrics::counter!(names::HTTP_REQUESTS_TOTAL, "route" => "/api/x", "status" => "200")
            .increment(3);
        metrics::histogram!(names::HTTP_REQUEST_DURATION_SECONDS, "route" => "/api/x").record(0.2);
        metrics::histogram!(names::JOB_DURATION_SECONDS, "kind" => "mock_agent_reply").record(0.05);
    });
    let text = registry.render();
    assert!(
        text.contains("http_requests_total{route=\"/api/x\",status=\"200\"} 3"),
        "{text}"
    );
    assert!(
        text.contains("http_request_duration_seconds_count{route=\"/api/x\"} 1"),
        "{text}"
    );
    assert!(
        text.contains("job_duration_seconds_count{kind=\"mock_agent_reply\"} 1"),
        "{text}"
    );
}

#[test]
fn descriptions_become_help_lines() {
    let text = MetricsRegistry::new("0").unwrap().render();
    assert!(text.contains("# HELP auth_lockouts_total"));
    assert!(text.contains("# TYPE db_pool_connections gauge"));
}

#[cfg(target_os = "linux")]
#[test]
fn process_metrics_read_proc() {
    let registry = MetricsRegistry::new("0").unwrap();
    metrics::with_local_recorder(registry.recorder(), crate::metrics::update_process_metrics);
    let text = registry.render();
    let memory_line = text
        .lines()
        .find(|line| line.starts_with("process_resident_memory_bytes "))
        .expect("memory gauge");
    let bytes: f64 = memory_line
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    assert!(bytes > 1_000_000.0);
    assert!(text.contains("process_cpu_seconds "));
}

#[test]
fn the_outbound_and_config_test_helpers_record_with_target_labels_only() {
    let registry = MetricsRegistry::new("0").unwrap();
    metrics::with_local_recorder(registry.recorder(), || {
        crate::metrics::record_outbound("provider", "ok", 0.3);
        crate::metrics::count_config_test("search", "reachable");
    });
    let text = registry.render();
    assert!(
        text.contains("outbound_requests_total{target=\"provider\",outcome=\"ok\"} 1"),
        "{text}"
    );
    assert!(
        text.contains("outbound_request_duration_seconds_count{target=\"provider\"} 1"),
        "{text}"
    );
    assert!(
        text.contains("config_test_total{target=\"search\",status=\"reachable\"} 1"),
        "{text}"
    );
}
