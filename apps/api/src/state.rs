//! Shared state for handlers and middleware.

use crate::pipeline::access::AccessKind;
use crate::pipeline::rate_limit::{Bucket, RateLimiter};
use crate::routes::ws::EventHub;
use axum::http::Method;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tandem_app::App;
use tandem_bootstrap::Shutdown;
use tandem_config::Config;
use tandem_telemetry::{MetricsHandle, Readiness};

/// What the pipeline needs to know about a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteInfo {
    /// Who may call it.
    pub access: AccessKind,
    /// Which rate-limit allowance it draws from.
    pub bucket: Bucket,
}

/// Route facts keyed by method and path template.
pub type RouteInfoMap = HashMap<(Method, String), RouteInfo>;

/// Everything a request handler can reach.
#[derive(Clone)]
pub struct AppState {
    /// The application services.
    pub app: App,
    /// Validated configuration.
    pub config: Arc<Config>,
    /// Readiness aggregation.
    pub readiness: Readiness,
    /// Metrics rendering.
    pub metrics: MetricsHandle,
    /// The per-source limiter.
    pub limiter: Arc<RateLimiter>,
    /// Route facts for the pipeline.
    pub routes: Arc<RouteInfoMap>,
    /// Wake-ups and per-user connection counts for the event feed.
    pub hub: EventHub,
    /// Process shutdown.
    pub shutdown: Shutdown,
    /// Where the built web client lives, when served by this process.
    pub web_dir: Option<PathBuf>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}
