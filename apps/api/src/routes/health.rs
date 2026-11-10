//! `/healthz`, `/readyz` and `/metrics` (LC-4, SE-10).

use crate::error::ApiError;
use crate::pipeline::csrf::constant_time_eq;
use crate::pipeline::rate_limit::Bucket;
use crate::routes::RouteTable;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub(super) fn register(table: RouteTable) -> RouteTable {
    table
        .public(Method::GET, "/healthz", Bucket::General, healthz)
        .public(Method::GET, "/readyz", Bucket::General, readyz)
        .public(Method::GET, "/metrics", Bucket::General, metrics)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[derive(Serialize)]
struct CheckBody {
    name: &'static str,
    ok: bool,
}

#[derive(Serialize)]
struct ReadyBody {
    ready: bool,
    checks: Vec<CheckBody>,
}

async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

async fn readyz(State(state): State<AppState>) -> Response {
    let report = state.readiness.report().await;
    for check in report.checks.iter().filter(|check| !check.ok) {
        tracing::warn!(
            probe = check.name,
            detail = check.detail.as_deref().unwrap_or(""),
            "readiness probe failed"
        );
    }
    let status = if report.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = ReadyBody {
        ready: report.ready,
        checks: report
            .checks
            .iter()
            .map(|check| CheckBody {
                name: check.name,
                ok: check.ok,
            })
            .collect(),
    };
    (status, Json(body)).into_response()
}

async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let Some(expected) = &state.config.metrics_token else {
        return Err(ApiError::not_found());
    };
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !constant_time_eq(supplied.as_bytes(), expected.expose().as_bytes()) {
        let mut response = ApiError::unauthenticated().into_response();
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return Ok(response);
    }
    let mut response = state.metrics.render().into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    Ok(response)
}
