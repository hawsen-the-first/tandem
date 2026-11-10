//! Logging with redaction, trace context, the metrics registry and readiness aggregation.

pub mod health;
pub mod logging;
pub mod metrics;
pub mod redact;
pub mod trace;

pub use health::{ProbeResult, Readiness, ReadinessProbe, ReadinessReport};
pub use logging::{InitError, JsonLogLayer, init_logging};
pub use metrics::{MetricsHandle, MetricsRegistry, names as metric_names, update_process_metrics};
pub use redact::{REDACTED, is_sensitive};
pub use trace::TraceContext;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-telemetry";

#[cfg(test)]
mod tests;
