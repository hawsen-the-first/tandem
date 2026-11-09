//! Liveness and readiness (LC-4). Probes are supplied by each process; this crate owns the
//! aggregation, the per-probe timeout and the shutdown flag.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// One dependency check.
#[async_trait]
pub trait ReadinessProbe: Send + Sync {
    /// A stable name, safe to show to operators.
    fn name(&self) -> &'static str;

    /// `Ok` when the dependency is healthy; `Err` carries a detail for the logs only.
    async fn check(&self) -> Result<(), String>;
}

/// The result of one probe. `detail` is for logs and never shown to callers of `/readyz`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    /// The probe name.
    pub name: &'static str,
    /// Whether it passed.
    pub ok: bool,
    /// Why it failed.
    pub detail: Option<String>,
}

/// The whole readiness answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessReport {
    /// All probes passed and the process is not shutting down.
    pub ready: bool,
    /// The process has begun shutting down.
    pub shutting_down: bool,
    /// Per-probe results (empty while shutting down).
    pub checks: Vec<ProbeResult>,
}

/// Aggregates probes.
#[derive(Clone)]
pub struct Readiness {
    probes: Vec<Arc<dyn ReadinessProbe>>,
    shutting_down: Arc<AtomicBool>,
    timeout: Duration,
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readiness")
            .field("probes", &self.probes.len())
            .finish_non_exhaustive()
    }
}

impl Readiness {
    /// Aggregate `probes`, each limited to `timeout`.
    pub fn new(probes: Vec<Arc<dyn ReadinessProbe>>, timeout: Duration) -> Self {
        Self {
            probes,
            shutting_down: Arc::new(AtomicBool::new(false)),
            timeout,
        }
    }

    /// Make every later report not-ready (503 during graceful shutdown).
    pub fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    /// Run the probes.
    pub async fn report(&self) -> ReadinessReport {
        if self.shutting_down.load(Ordering::SeqCst) {
            return ReadinessReport {
                ready: false,
                shutting_down: true,
                checks: Vec::new(),
            };
        }
        let mut checks = Vec::with_capacity(self.probes.len());
        for probe in &self.probes {
            let outcome = tokio::time::timeout(self.timeout, probe.check()).await;
            let (ok, detail) = match outcome {
                Ok(Ok(())) => (true, None),
                Ok(Err(detail)) => (false, Some(detail)),
                Err(_) => (false, Some("timed out".to_owned())),
            };
            checks.push(ProbeResult {
                name: probe.name(),
                ok,
                detail,
            });
        }
        ReadinessReport {
            ready: checks.iter().all(|check| check.ok),
            shutting_down: false,
            checks,
        }
    }
}
