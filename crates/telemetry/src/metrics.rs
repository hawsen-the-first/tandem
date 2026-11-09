//! The metrics registry: every OB-1 metric, rendered in the Prometheus text format (LC-3).
//! Recording never fails and never affects a request; the `metrics` macros are no-ops when no
//! recorder is installed.

use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::{
    BuildError, PrometheusBuilder, PrometheusHandle, PrometheusRecorder,
};

/// Metric names, so call sites and tests cannot drift apart.
pub mod names {
    /// Requests by route template and status.
    pub const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
    /// Request latency by route template.
    pub const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";
    /// Sign-in attempts by outcome.
    pub const AUTH_SIGN_IN_TOTAL: &str = "auth_sign_in_total";
    /// Accounts locked out.
    pub const AUTH_LOCKOUTS_TOTAL: &str = "auth_lockouts_total";
    /// Authorisation denials.
    pub const AUTHZ_DENIED_TOTAL: &str = "authz_denied_total";
    /// Requests refused by the rate limiter, by bucket.
    pub const RATE_LIMITED_TOTAL: &str = "rate_limited_total";
    /// Database pool connections by state (`in_use`, `idle`, `size`).
    pub const DB_POOL_CONNECTIONS: &str = "db_pool_connections";
    /// Database query latency.
    pub const DB_QUERY_DURATION_SECONDS: &str = "db_query_duration_seconds";
    /// Unpublished outbox events.
    pub const OUTBOX_PENDING: &str = "outbox_pending";
    /// Age of the oldest unpublished outbox event.
    pub const OUTBOX_RELAY_LAG_SECONDS: &str = "outbox_relay_lag_seconds";
    /// Queued jobs.
    pub const JOB_QUEUE_DEPTH: &str = "job_queue_depth";
    /// Age of the oldest due job.
    pub const JOB_OLDEST_AGE_SECONDS: &str = "job_oldest_age_seconds";
    /// Job run time by kind.
    pub const JOB_DURATION_SECONDS: &str = "job_duration_seconds";
    /// Open WebSocket connections.
    pub const WS_CONNECTIONS: &str = "ws_connections";
    /// Password hashes running now.
    pub const HASH_INFLIGHT: &str = "hash_inflight";
    /// Time spent waiting for a hashing slot.
    pub const HASH_WAIT_SECONDS: &str = "hash_wait_seconds";
    /// Sign-ins refused because the hasher was busy.
    pub const HASH_REJECTED_TOTAL: &str = "hash_rejected_total";
    /// Breach checks that fell back to the offline filter.
    pub const BREACH_CHECK_DEGRADED_TOTAL: &str = "breach_check_degraded_total";
    /// Log events by level.
    pub const LOG_EVENTS_TOTAL: &str = "log_events_total";
    /// Always 1, labelled with the version.
    pub const BUILD_INFO: &str = "build_info";
    /// Resident memory of the process.
    pub const PROCESS_RESIDENT_MEMORY_BYTES: &str = "process_resident_memory_bytes";
    /// CPU time used by the process.
    pub const PROCESS_CPU_SECONDS: &str = "process_cpu_seconds";
}

const LATENCY_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Owns the recorder and renders it.
#[derive(Debug)]
pub struct MetricsRegistry {
    recorder: PrometheusRecorder,
    handle: PrometheusHandle,
}

impl MetricsRegistry {
    /// Create a registry with every metric described and the unlabelled ones present at zero.
    ///
    /// # Errors
    /// [`BuildError`] if the histogram buckets are rejected (a programming error).
    pub fn new(version: &str) -> Result<Self, BuildError> {
        let recorder = PrometheusBuilder::new()
            .set_buckets(LATENCY_BUCKETS)?
            .build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            describe_all();
            register_defaults(version);
        });
        Ok(Self { recorder, handle })
    }

    /// The recorder, for recording into this registry without installing it globally (tests).
    pub fn recorder(&self) -> &PrometheusRecorder {
        &self.recorder
    }

    /// The Prometheus text exposition.
    pub fn render(&self) -> String {
        self.handle.run_upkeep();
        self.handle.render()
    }

    /// Install as the process-wide recorder and return a handle for rendering.
    ///
    /// # Errors
    /// [`metrics::SetRecorderError`] if a recorder is already installed.
    pub fn install(self) -> Result<MetricsHandle, metrics::SetRecorderError<PrometheusRecorder>> {
        let handle = self.handle.clone();
        metrics::set_global_recorder(self.recorder)?;
        Ok(MetricsHandle { handle })
    }
}

/// Renders the installed registry.
#[derive(Debug, Clone)]
pub struct MetricsHandle {
    handle: PrometheusHandle,
}

impl MetricsHandle {
    /// The Prometheus text exposition, with process gauges refreshed.
    pub fn render(&self) -> String {
        update_process_metrics();
        self.handle.run_upkeep();
        self.handle.render()
    }
}

fn describe_all() {
    use names::*;
    describe_counter!(HTTP_REQUESTS_TOTAL, "HTTP requests by route and status");
    describe_histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        Unit::Seconds,
        "HTTP request latency by route"
    );
    describe_counter!(AUTH_SIGN_IN_TOTAL, "Sign-in attempts by outcome");
    describe_counter!(
        AUTH_LOCKOUTS_TOTAL,
        "Accounts locked out after repeated failures"
    );
    describe_counter!(AUTHZ_DENIED_TOTAL, "Requests denied by authorisation");
    describe_counter!(
        RATE_LIMITED_TOTAL,
        "Requests refused by the rate limiter, by bucket"
    );
    describe_gauge!(DB_POOL_CONNECTIONS, "Database pool connections by state");
    describe_histogram!(
        DB_QUERY_DURATION_SECONDS,
        Unit::Seconds,
        "Database query latency"
    );
    describe_gauge!(OUTBOX_PENDING, "Outbox events not yet published");
    describe_gauge!(
        OUTBOX_RELAY_LAG_SECONDS,
        Unit::Seconds,
        "Age of the oldest unpublished outbox event"
    );
    describe_gauge!(JOB_QUEUE_DEPTH, "Jobs waiting to run");
    describe_gauge!(
        JOB_OLDEST_AGE_SECONDS,
        Unit::Seconds,
        "Age of the oldest due job"
    );
    describe_histogram!(JOB_DURATION_SECONDS, Unit::Seconds, "Job run time by kind");
    describe_gauge!(WS_CONNECTIONS, "Open WebSocket connections");
    describe_gauge!(HASH_INFLIGHT, "Password hashes running now");
    describe_histogram!(HASH_WAIT_SECONDS, Unit::Seconds, "Wait for a hashing slot");
    describe_counter!(
        HASH_REJECTED_TOTAL,
        "Sign-ins refused because the hasher was busy"
    );
    describe_counter!(
        BREACH_CHECK_DEGRADED_TOTAL,
        "Breach checks that used the offline filter only"
    );
    describe_counter!(LOG_EVENTS_TOTAL, "Log events by level");
    describe_gauge!(BUILD_INFO, "Build information (always 1)");
    describe_gauge!(
        PROCESS_RESIDENT_MEMORY_BYTES,
        Unit::Bytes,
        "Resident memory of the process"
    );
    describe_gauge!(
        PROCESS_CPU_SECONDS,
        Unit::Seconds,
        "CPU time used by the process"
    );
}

fn register_defaults(version: &str) {
    use names::*;
    for name in [
        AUTH_LOCKOUTS_TOTAL,
        AUTHZ_DENIED_TOTAL,
        HASH_REJECTED_TOTAL,
        BREACH_CHECK_DEGRADED_TOTAL,
    ] {
        metrics::counter!(name).increment(0);
    }
    for outcome in [
        "success",
        "bad_credentials",
        "locked",
        "mfa_required",
        "busy",
    ] {
        metrics::counter!(AUTH_SIGN_IN_TOTAL, "outcome" => outcome).increment(0);
    }
    for name in [
        OUTBOX_PENDING,
        OUTBOX_RELAY_LAG_SECONDS,
        JOB_QUEUE_DEPTH,
        JOB_OLDEST_AGE_SECONDS,
        WS_CONNECTIONS,
        HASH_INFLIGHT,
    ] {
        metrics::gauge!(name).set(0.0);
    }
    for state in ["in_use", "idle", "size"] {
        metrics::gauge!(DB_POOL_CONNECTIONS, "state" => state).set(0.0);
    }
    metrics::gauge!(BUILD_INFO, "version" => version.to_owned()).set(1.0);
    metrics::histogram!(DB_QUERY_DURATION_SECONDS).record(0.0);
    metrics::histogram!(HASH_WAIT_SECONDS).record(0.0);
}

/// Refresh the process gauges from `/proc` (Linux; a no-op elsewhere). Assumes 4 KiB pages and a
/// 100 Hz clock tick, which holds on every platform Tandem's container images target.
pub fn update_process_metrics() {
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm")
        && let Some(pages) = statm
            .split_whitespace()
            .nth(1)
            .and_then(|text| text.parse::<f64>().ok())
    {
        metrics::gauge!(names::PROCESS_RESIDENT_MEMORY_BYTES).set(pages * 4096.0);
    }
    if let Ok(stat) = std::fs::read_to_string("/proc/self/stat")
        && let Some(after_name) = stat.rsplit_once(')').map(|(_, rest)| rest)
    {
        let fields: Vec<&str> = after_name.split_whitespace().collect();
        let ticks = |index: usize| {
            fields
                .get(index)
                .and_then(|text| text.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        metrics::gauge!(names::PROCESS_CPU_SECONDS).set((ticks(11) + ticks(12)) / 100.0);
    }
}

/// Set a gauge that carries a `state` label (a small helper so callers need not depend on the
/// `metrics` crate).
pub fn set_gauge_with_state(name: &'static str, state: &'static str, value: f64) {
    metrics::gauge!(name, "state" => state).set(value);
}
