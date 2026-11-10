//! The worker's small HTTP surface: `/healthz`, `/readyz` and `/metrics` (SE-10, LC-4).

use async_trait::async_trait;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use std::time::Duration;
use tandem_config::Secret;
use tandem_store_postgres::PgStore;
use tandem_telemetry::{MetricsHandle, Readiness, ReadinessProbe};
use time::OffsetDateTime;

/// The relay is considered stuck when the oldest unpublished event is older than this.
pub const MAX_RELAY_LAG: Duration = Duration::from_secs(30);

/// Compare two byte strings without stopping at the first difference.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(a ^ b);
    }
    difference == 0
}

/// Fails when the outbox relay has fallen behind.
#[derive(Debug, Clone)]
pub struct RelayLagProbe {
    store: PgStore,
    max_lag: Duration,
}

impl RelayLagProbe {
    /// Probe `store`, failing beyond `max_lag`.
    pub fn new(store: PgStore, max_lag: Duration) -> Self {
        Self { store, max_lag }
    }
}

#[async_trait]
impl ReadinessProbe for RelayLagProbe {
    fn name(&self) -> &'static str {
        "relay_lag"
    }

    async fn check(&self) -> Result<(), String> {
        let stats = self
            .store
            .outbox_stats(OffsetDateTime::now_utc())
            .await
            .map_err(|error| error.to_string())?;
        match stats.oldest_age {
            Some(age) if age.as_seconds_f64() > self.max_lag.as_secs_f64() => Err(format!(
                "the oldest unpublished event is {} s old",
                age.whole_seconds()
            )),
            _ => Ok(()),
        }
    }
}

/// What the handlers need.
#[derive(Clone)]
pub struct HealthState {
    /// The readiness aggregator.
    pub readiness: Readiness,
    /// The installed metrics registry.
    pub metrics: MetricsHandle,
    /// The `/metrics` bearer token; the endpoint is disabled without one.
    pub metrics_token: Option<Secret>,
}

impl std::fmt::Debug for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthState").finish_non_exhaustive()
    }
}

/// The routes.
pub fn router(state: HealthState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics_endpoint))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(state): State<HealthState>) -> (StatusCode, Json<Value>) {
    let report = state.readiness.report().await;
    for check in report.checks.iter().filter(|check| !check.ok) {
        tracing::warn!(
            probe = check.name,
            detail = check.detail.as_deref().unwrap_or(""),
            "not ready"
        );
    }
    let status = if report.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let checks: Vec<Value> = report
        .checks
        .iter()
        .map(|check| json!({ "name": check.name, "ok": check.ok }))
        .collect();
    (
        status,
        Json(
            json!({ "ready": report.ready, "shutting_down": report.shutting_down, "checks": checks }),
        ),
    )
}

async fn metrics_endpoint(State(state): State<HealthState>, headers: HeaderMap) -> Response {
    let Some(expected) = &state.metrics_token else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    match presented {
        None => StatusCode::UNAUTHORIZED.into_response(),
        Some(token) if constant_time_eq(token.as_bytes(), expected.expose().as_bytes()) => (
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            state.metrics.render(),
        )
            .into_response(),
        Some(_) => StatusCode::FORBIDDEN.into_response(),
    }
}
