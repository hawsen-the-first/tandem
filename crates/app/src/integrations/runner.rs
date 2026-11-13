//! The test runner (LC-32): executes a queued `config_test` job. One run per target holds an
//! advisory lock for its whole life, results are stored as they arrive, and nothing the remote
//! side said is kept (NFR design pattern 2; RL-8, RL-9, PF-8).

use crate::deps::App;
use crate::error::AppError;
use async_trait::async_trait;
use std::sync::{Mutex, PoisonError};
use std::time::Duration as StdDuration;
use tandem_core::audit::{AuditEvent, AuditEventType, AuditOutcome};
use tandem_core::config::{
    FailureReason, ProbeResult, ProbeSubject, RunStatus, TestRun, TestStatus, TestTarget,
};
use tandem_core::ids::{JobId, TestRunId};
use tandem_core::job::{Job, JobKind};
use tandem_ports::{ModelToProbe, ResultSink};
use time::{Duration, OffsetDateTime};

/// A hard ceiling on one run, a little above the adapters' own 60 second budget.
const RUN_BUDGET: StdDuration = StdDuration::from_secs(75);
/// How long to wait before trying again when another worker holds the target's lock.
const BUSY_RETRY: Duration = Duration::seconds(5);
/// A run still queued after this long has no worker to run it.
const QUEUED_LIMIT: Duration = Duration::seconds(120);

/// Stores each result on the run as the probe produces it.
struct RunSink<'app> {
    app: &'app App,
    run_id: TestRunId,
    failure: Mutex<Option<AppError>>,
}

impl RunSink<'_> {
    fn take_failure(&self) -> Option<AppError> {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    async fn store(&self, result: &ProbeResult) -> Result<(), AppError> {
        let app = self.app;
        let mut tx = app.store.begin().await?;
        let mut run = tx
            .test_runs()
            .get(self.run_id)
            .await?
            .ok_or(AppError::NotFound)?;
        run.record(result.clone())
            .map_err(|error| AppError::Internal(error.to_string()))?;
        tx.test_runs().save(&run).await?;
        if result.status == TestStatus::Ok {
            match result.subject {
                ProbeSubject::Model(model_id) => {
                    if let Some(tool_calling) = result.details.tool_calling {
                        tx.providers()
                            .set_detected(model_id, tool_calling, result.details.context_tokens)
                            .await?;
                    }
                }
                ProbeSubject::Connector => {
                    if let TestTarget::Connector(kind) = run.target {
                        tx.connectors()
                            .set_identity(
                                kind,
                                result.details.account_login.as_deref(),
                                result.details.token_expires_at,
                            )
                            .await?;
                    }
                }
                _ => {}
            }
        }
        tx.commit().await?;
        app.config_cache.clear();
        tandem_telemetry::metrics::count_config_test(
            run.target.kind_name(),
            result.status.as_str(),
        );
        Ok(())
    }
}

#[async_trait]
impl ResultSink for RunSink<'_> {
    async fn record(&self, result: ProbeResult) {
        if let Err(error) = self.store(&result).await {
            self.failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(error);
        }
    }
}

impl App {
    /// Run a queued configuration test (the `config_test` job handler).
    pub(crate) async fn run_config_test(&self, run_id: TestRunId) -> Result<(), AppError> {
        let run = {
            let mut tx = self.store.begin().await?;
            tx.test_runs().get(run_id).await?
        };
        let Some(run) = run else { return Ok(()) };
        if !run.is_active() {
            return Ok(());
        }
        let Some(guard) = self.integrations.locks.try_lock(&run.target.key()).await? else {
            return self.requeue_after_busy(run_id).await;
        };
        let outcome = self.execute_run(run_id).await;
        guard.release().await;
        outcome
    }

    async fn requeue_after_busy(&self, run_id: TestRunId) -> Result<(), AppError> {
        let now = self.clock.now();
        let job = Job::new(
            JobId::generate(&*self.clock, &*self.rng)?,
            JobKind::ConfigTest { run_id },
            now + BUSY_RETRY,
            None,
        );
        let mut tx = self.store.begin().await?;
        tx.jobs().enqueue(&job).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn execute_run(&self, run_id: TestRunId) -> Result<(), AppError> {
        let target = {
            let mut tx = self.store.begin().await?;
            let Some(mut run) = tx.test_runs().get(run_id).await? else {
                return Ok(());
            };
            match run.status {
                RunStatus::Queued => {}
                // A worker died mid-run; start again from nothing (D2).
                RunStatus::Running => run
                    .requeue()
                    .map_err(|error| AppError::Internal(error.to_string()))?,
                RunStatus::Succeeded | RunStatus::Failed => return Ok(()),
            }
            run.start(self.clock.now())
                .map_err(|error| AppError::Internal(error.to_string()))?;
            tx.test_runs().save(&run).await?;
            tx.commit().await?;
            run.target
        };

        let sink = RunSink {
            app: self,
            run_id,
            failure: Mutex::new(None),
        };
        let probed = tokio::time::timeout(RUN_BUDGET, self.probe_target(target, &sink)).await;
        match probed {
            Err(_) => {
                return Err(AppError::Internal(
                    "a configuration test exceeded its time budget".to_owned(),
                ));
            }
            Ok(Err(error)) => return Err(error),
            Ok(Ok(())) => {}
        }
        if let Some(error) = sink.take_failure() {
            return Err(error);
        }

        let finished = {
            let mut tx = self.store.begin().await?;
            let Some(mut run) = tx.test_runs().get(run_id).await? else {
                return Ok(());
            };
            run.finish(self.clock.now())
                .map_err(|error| AppError::Internal(error.to_string()))?;
            tx.test_runs().save(&run).await?;
            tx.commit().await?;
            run
        };
        self.audit_test_run(&finished).await;
        Ok(())
    }

    async fn probe_target(&self, target: TestTarget, sink: &RunSink<'_>) -> Result<(), AppError> {
        // Fresh from the database, not the cache: a test must use what was just saved.
        let snapshot = self.load_snapshot().await?;
        match target {
            TestTarget::Provider(id) => {
                let Some(record) = snapshot
                    .providers
                    .iter()
                    .find(|record| record.provider.id == id)
                else {
                    sink.record(ProbeResult::new(
                        ProbeSubject::Provider,
                        TestStatus::NotConfigured,
                    ))
                    .await;
                    return Ok(());
                };
                let connection = self.provider_connection(record)?;
                let models: Vec<ModelToProbe> = record
                    .models
                    .iter()
                    .map(|model| ModelToProbe {
                        id: model.id,
                        name: model.name.clone(),
                    })
                    .collect();
                self.integrations
                    .provider_probe
                    .probe(&connection, &models, sink)
                    .await;
            }
            TestTarget::Search(kind) => {
                let Some(record) = snapshot
                    .search
                    .iter()
                    .find(|record| record.config.kind == kind)
                else {
                    sink.record(ProbeResult::new(
                        ProbeSubject::Search,
                        TestStatus::NotConfigured,
                    ))
                    .await;
                    return Ok(());
                };
                let connection = self.search_connection(record)?;
                sink.record(self.integrations.search_probe.test(&connection).await)
                    .await;
            }
            TestTarget::Connector(_) => {
                let Some(connector) = &snapshot.connector else {
                    sink.record(ProbeResult::new(
                        ProbeSubject::Connector,
                        TestStatus::NotConfigured,
                    ))
                    .await;
                    return Ok(());
                };
                let connection = self.connector_connection(connector)?;
                sink.record(self.integrations.connector_probe.test(&connection).await)
                    .await;
            }
            TestTarget::Runtime => {
                sink.record(self.integrations.runtime_probe.probe().await)
                    .await;
            }
        }
        Ok(())
    }

    /// Audit a finished run after the fact. A failed write is logged and the result stands (RL-9).
    async fn audit_test_run(&self, run: &TestRun) {
        let outcome = if run.status == RunStatus::Succeeded
            && run
                .results
                .iter()
                .all(|result| result.status == TestStatus::Ok)
        {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failure
        };
        let written: Result<(), AppError> = async {
            let mut tx = self.store.begin().await?;
            let requester = tx.users().get(run.requested_by).await?;
            let mut event =
                AuditEvent::new(AuditEventType::ConfigTestRun, outcome, self.clock.now())
                    .object("test_run", run.id)
                    .detail("target", run.target.key())
                    .detail("run_status", run.status.as_str());
            event = match requester {
                Some(user) => event.actor(user.id, user.display_name.as_str()),
                None => event.actor_label("unknown user"),
            };
            for result in &run.results {
                event = event.detail(
                    &format!("result_{}", result.subject.key()),
                    result.status.as_str(),
                );
            }
            if let Some(failure) = run.failure {
                event = event.detail("failure", failure.as_str());
            }
            self.append_audit(tx.as_mut(), event).await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        if let Err(error) = written {
            tracing::error!(%error, run_id = %run.id, "could not audit a finished configuration test");
        }
    }

    /// The queue gave a `config_test` job another attempt or gave up on it: mirror that on the run
    /// (D2). A retry starts again from nothing; the last failed attempt fails the run.
    pub(crate) async fn config_test_job_failed(
        &self,
        run_id: TestRunId,
        will_retry: bool,
    ) -> Result<(), AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let Some(mut run) = tx.test_runs().get(run_id).await? else {
            return Ok(());
        };
        if will_retry {
            if run.status == RunStatus::Running {
                run.requeue()
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                tx.test_runs().save(&run).await?;
                tx.commit().await?;
            }
            return Ok(());
        }
        if !run.is_active() {
            return Ok(());
        }
        run.fail(now, FailureReason::CouldNotComplete)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        tx.test_runs().save(&run).await?;
        tx.commit().await?;
        self.audit_test_run(&run).await;
        Ok(())
    }

    /// Fail runs nobody picked up within two minutes so the screen stops waiting and the target
    /// can be tested again ("No worker is available"). Returns how many.
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn fail_stuck_test_runs(&self) -> Result<u64, AppError> {
        let now = self.clock.now();
        let cutoff: OffsetDateTime = now - QUEUED_LIMIT;
        let mut tx = self.store.begin().await?;
        let stuck = tx.test_runs().queued_before(cutoff).await?;
        let mut failed = Vec::new();
        for mut run in stuck {
            if run.fail(now, FailureReason::NoWorkerAvailable).is_ok() {
                tx.test_runs().save(&run).await?;
                failed.push(run);
            }
        }
        tx.commit().await?;
        for run in &failed {
            self.audit_test_run(run).await;
        }
        Ok(u64::try_from(failed.len()).unwrap_or(u64::MAX))
    }
}
