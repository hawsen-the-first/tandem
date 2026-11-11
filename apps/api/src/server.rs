//! Assembling the router and serving it.

use crate::pipeline::rate_limit::RateLimiter;
use crate::pipeline::{
    access_log, body_limit, catch_panic_response, csrf_layer, rate_limit_layer, request_identity,
    security_headers, session_layer,
};
use crate::routes::ws::EventHub;
use crate::routes::{RouteError, all_routes, assets};
use crate::state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tandem_app::App;
use tandem_bootstrap::Shutdown;
use tandem_config::Config;
use tandem_telemetry::{MetricsHandle, Readiness};
use tokio::net::TcpListener;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::timeout::TimeoutLayer;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const LIMITER_ENTRIES: usize = 10_000;

/// Everything `build_state` needs from the outside.
#[derive(Debug)]
pub struct StateInputs {
    /// The application.
    pub app: App,
    /// Configuration.
    pub config: Arc<Config>,
    /// Readiness aggregation.
    pub readiness: Readiness,
    /// Metrics rendering.
    pub metrics: MetricsHandle,
    /// Process shutdown.
    pub shutdown: Shutdown,
    /// The built client directory, when this process serves it.
    pub web_dir: Option<PathBuf>,
}

/// Build the router and the state it runs with.
///
/// # Errors
/// [`RouteError`] when the route table is inconsistent (the start-up check).
pub fn build(inputs: StateInputs) -> Result<(Router, AppState), RouteError> {
    let (routes, info) = all_routes()?;
    let state = AppState {
        app: inputs.app,
        config: inputs.config,
        readiness: inputs.readiness,
        metrics: inputs.metrics,
        limiter: Arc::new(RateLimiter::new(LIMITER_ENTRIES)),
        routes: Arc::new(info),
        hub: EventHub::new(),
        shutdown: inputs.shutdown,
        web_dir: inputs.web_dir,
    };
    let router = routes
        .fallback(assets::fallback)
        .layer(from_fn_with_state(state.clone(), csrf_layer))
        .layer(from_fn_with_state(state.clone(), session_layer))
        .layer(from_fn_with_state(state.clone(), rate_limit_layer))
        .layer(axum::middleware::from_fn(body_limit))
        .layer(DefaultBodyLimit::max(crate::pipeline::BODY_LIMIT_BYTES))
        .layer(from_fn_with_state(state.clone(), security_headers))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(CatchPanicLayer::custom(catch_panic_response))
        .layer(axum::middleware::from_fn(access_log))
        .layer(from_fn_with_state(state.clone(), request_identity))
        .with_state(state.clone());
    Ok((router, state))
}

/// Serve until `shutdown` fires, then let in-flight requests finish.
///
/// # Errors
/// [`std::io::Error`] if the server fails.
pub async fn serve(
    listener: TcpListener,
    router: Router,
    shutdown: Shutdown,
) -> std::io::Result<()> {
    let mut stop = shutdown.subscribe();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = stop.wait_for(|stopping| *stopping).await;
    })
    .await
}
