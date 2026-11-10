//! The middleware functions of the pipeline.

use crate::cookie;
use crate::error::ApiError;
use crate::pipeline::SessionInfo;
use crate::pipeline::access::AccessKind;
use crate::pipeline::csrf::{constant_time_eq, csrf_token_for, is_unsafe_method};
use crate::pipeline::rate_limit::{Bucket, Decision};
use crate::pipeline::source::{ClientAddr, is_valid_request_id, peer_ip, resolve_client};
use crate::state::{AppState, RouteInfo};
use axum::body::Body;
use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;
use std::time::Instant;
use tandem_app::AppError;
use tandem_support::{SystemRng, random_array};
use tandem_telemetry::{TraceContext, metric_names};
use tracing::Instrument;

const REQUEST_ID_HEADER: &str = "x-request-id";
const CSRF_HEADER: &str = "x-csrf-token";
/// Largest request body accepted (SE-6).
pub const BODY_LIMIT_BYTES: usize = 1_000_000;

/// Request identity placed in the extensions by [`request_identity`].
#[derive(Debug, Clone)]
pub struct RequestMeta {
    /// The correlation ID.
    pub request_id: String,
    /// The trace context to hand to services, as a `traceparent` value.
    pub traceparent: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn route_info(state: &AppState, request: &Request) -> Option<RouteInfo> {
    let path = request
        .extensions()
        .get::<MatchedPath>()?
        .as_str()
        .to_owned();
    state.routes.get(&(request.method().clone(), path)).copied()
}

/// Layer 1: the client address, the request ID and trace context, and the request span.
pub async fn request_identity(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let peer = peer_ip(
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|connect| connect.0),
    );
    let forwarded = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let client = resolve_client(peer, forwarded.as_deref(), &state.config.trusted_proxies);

    let incoming_id = client
        .via_trusted_proxy
        .then(|| {
            request
                .headers()
                .get(REQUEST_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .filter(|candidate| is_valid_request_id(candidate))
                .map(str::to_owned)
        })
        .flatten();
    let rng = SystemRng;
    let request_id = incoming_id.unwrap_or_else(|| {
        random_array::<16>(&rng).map_or_else(|_| "unavailable".to_owned(), |bytes| hex(&bytes))
    });

    let incoming_trace = client
        .via_trusted_proxy
        .then(|| {
            request
                .headers()
                .get("traceparent")
                .and_then(|value| value.to_str().ok())
                .and_then(TraceContext::parse)
        })
        .flatten();
    let trace = match incoming_trace {
        Some(parent) => parent.child(&rng).ok(),
        None => TraceContext::new_root(&rng).ok(),
    };

    request.extensions_mut().insert(client);
    request.extensions_mut().insert(RequestMeta {
        request_id: request_id.clone(),
        traceparent: trace.map(|trace| trace.to_header()),
    });
    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        trace_id = %trace.map(|trace| trace.trace_id()).unwrap_or_default(),
        user_id = tracing::field::Empty,
    );
    let mut response = next.run(request).instrument(span).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

/// Layer 2: one JSON access-log line and the request metrics (SE-3, OB-1).
pub async fn access_log(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |path| path.as_str().to_owned());
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let elapsed = started.elapsed();
    metrics_record(&route, status, elapsed.as_secs_f64());
    tracing::info!(
        method = %method,
        route = %route,
        status,
        duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        "request"
    );
    response
}

fn metrics_record(route: &str, status: u16, seconds: f64) {
    tandem_telemetry::metrics::record_http_request(
        metric_names::HTTP_REQUESTS_TOTAL,
        metric_names::HTTP_REQUEST_DURATION_SECONDS,
        route,
        status,
        seconds,
    );
}

/// Layer 3 response builder: a panic becomes a generic 500 (the detail goes to the log only).
pub fn catch_panic_response(
    panic: Box<dyn std::any::Any + Send + 'static>,
) -> axum::http::Response<Body> {
    let detail = panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-text panic payload".to_owned());
    tracing::error!(panic = %detail, "a request handler panicked");
    ApiError::internal().into_response()
}

/// Layer 4: security headers on every response (SE-5).
pub async fn security_headers(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let is_api = request.uri().path().starts_with("/api")
        || matches!(request.uri().path(), "/healthz" | "/readyz" | "/metrics");
    let mut response = next.run(request).await;
    let websocket_origin = state.config.public_websocket_origin();
    let policy = format!(
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
         connect-src 'self' {websocket_origin}; font-src 'self'; frame-ancestors 'none'; \
         base-uri 'none'; form-action 'self'; object-src 'none'"
    );
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&policy) {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    if is_api && !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// Layer 5: per-source token buckets (SE-12).
pub async fn rate_limit_layer(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let bucket = route_info(&state, &request).map_or(Bucket::General, |info| info.bucket);
    let source = request
        .extensions()
        .get::<ClientAddr>()
        .map_or_else(|| std::net::IpAddr::from([0, 0, 0, 0]), |client| client.ip);
    match state.limiter.check(source, bucket, Instant::now()) {
        Decision::Allowed => next.run(request).await,
        Decision::Retry(wait) => {
            tandem_telemetry::metrics::count_rate_limited(
                metric_names::RATE_LIMITED_TOTAL,
                bucket.label(),
            );
            ApiError::rate_limited(wait.as_secs() + 1).into_response()
        }
    }
}

/// Layer 6: refuse oversized bodies before they are read (SE-6).
pub async fn body_limit(request: Request, next: Next) -> Response {
    let declared = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.parse::<usize>().ok());
    if declared.is_some_and(|length| length > BODY_LIMIT_BYTES) {
        return ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "That request is too large.",
        )
        .into_response();
    }
    next.run(request).await
}

/// Layer 7: resolve the session cookie from the database on every request (BR-9.1). Public routes
/// do not need it. A database failure is a 500, never an anonymous pass.
pub async fn session_layer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let needs_session =
        route_info(&state, &request).is_none_or(|info| info.access != AccessKind::Public);
    if needs_session && let Some(token) = cookie::session_token(request.headers()) {
        match state.app.resolve_session(&token).await {
            Ok(authenticated) => {
                tracing::Span::current()
                    .record("user_id", authenticated.actor.user_id.to_string().as_str());
                request.extensions_mut().insert(SessionInfo {
                    actor: authenticated.actor,
                    user: authenticated.user,
                    mfa_enrolled: authenticated.mfa_enrolled,
                    token,
                });
            }
            Err(AppError::Unauthenticated) => {}
            Err(error) => return ApiError::from(error).into_response(),
        }
    }
    next.run(request).await
}

/// Layer 8: Origin and CSRF checks for state-changing requests (SE-6).
pub async fn csrf_layer(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !is_unsafe_method(request.method()) {
        return next.run(request).await;
    }
    let access = route_info(&state, &request).map_or(AccessKind::AnyUser, |info| info.access);
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let expected = state.config.public_origin();
    match origin {
        Some(origin) if origin != expected => return forbidden("bad_origin"),
        None if access != AccessKind::Public => return forbidden("missing_origin"),
        _ => {}
    }
    if access != AccessKind::Public
        && let Some(session) = request.extensions().get::<SessionInfo>()
    {
        let supplied = request
            .headers()
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !constant_time_eq(
            supplied.as_bytes(),
            csrf_token_for(&session.token).as_bytes(),
        ) {
            return forbidden("csrf");
        }
    }
    next.run(request).await
}

fn forbidden(code: &str) -> Response {
    ApiError::new(StatusCode::FORBIDDEN, code, "That request was refused.").into_response()
}
