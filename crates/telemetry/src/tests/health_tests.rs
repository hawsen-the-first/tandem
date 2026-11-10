use crate::health::{Readiness, ReadinessProbe};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

struct Fixed(&'static str, Result<(), &'static str>);

#[async_trait]
impl ReadinessProbe for Fixed {
    fn name(&self) -> &'static str {
        self.0
    }
    async fn check(&self) -> Result<(), String> {
        self.1.map_err(str::to_owned)
    }
}

struct Slow;

#[async_trait]
impl ReadinessProbe for Slow {
    fn name(&self) -> &'static str {
        "slow"
    }
    async fn check(&self) -> Result<(), String> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok(())
    }
}

fn readiness(probes: Vec<Arc<dyn ReadinessProbe>>) -> Readiness {
    Readiness::new(probes, Duration::from_secs(1))
}

#[tokio::test]
async fn all_probes_passing_is_ready() {
    let report = readiness(vec![Arc::new(Fixed("database", Ok(())))])
        .report()
        .await;
    assert!(report.ready);
    assert!(!report.shutting_down);
    assert_eq!(report.checks.len(), 1);
}

#[tokio::test]
async fn one_failing_probe_is_not_ready_and_keeps_its_detail() {
    let report = readiness(vec![
        Arc::new(Fixed("database", Ok(()))),
        Arc::new(Fixed("vault", Err("canary mismatch"))),
    ])
    .report()
    .await;
    assert!(!report.ready);
    assert!(report.checks[0].ok);
    assert!(!report.checks[1].ok);
    assert_eq!(report.checks[1].detail.as_deref(), Some("canary mismatch"));
}

#[tokio::test(start_paused = true)]
async fn a_slow_probe_times_out() {
    let report = readiness(vec![Arc::new(Slow)]).report().await;
    assert!(!report.ready);
    assert_eq!(report.checks[0].detail.as_deref(), Some("timed out"));
}

#[tokio::test]
async fn shutdown_makes_every_later_report_not_ready_without_probing() {
    let readiness = readiness(vec![Arc::new(Fixed("database", Ok(())))]);
    assert!(readiness.report().await.ready);
    readiness.begin_shutdown();
    let report = readiness.report().await;
    assert!(!report.ready);
    assert!(report.shutting_down);
    assert!(report.checks.is_empty());
}
