//! Starting and reading configuration tests (US-2.3, 2.5, 2.6, 2.9; RL-8).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::config::{TestRun, TestTarget};
use tandem_core::ids::{JobId, TestRunId};
use tandem_core::job::{Job, JobKind};
use tandem_core::permissions::Action;
use tandem_ports::StoreError;

impl App {
    /// Queue a test and its job in one transaction (W17, W20, W21, W23). Only one test runs per
    /// target: a second request answers [`AppError::AlreadyRunning`] with the run in progress so the
    /// screen can attach to it.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins or a provider that does not exist;
    /// [`AppError::AlreadyRunning`].
    pub async fn start_test(
        &self,
        ctx: &RequestContext,
        target: TestTarget,
    ) -> Result<TestRun, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let run = TestRun::queued(
            TestRunId::generate(&*self.clock, &*self.rng)?,
            target,
            actor.user_id,
            now,
        );
        let job = Job::new(
            JobId::generate(&*self.clock, &*self.rng)?,
            JobKind::ConfigTest { run_id: run.id },
            now,
            ctx.trace_context.clone(),
        );
        let inserted = {
            let mut tx = self.store.begin().await?;
            if let TestTarget::Provider(id) = target
                && tx.providers().get(id).await?.is_none()
            {
                return Err(AppError::NotFound);
            }
            if let Some(active) = tx.test_runs().active_for(target).await? {
                return Err(AppError::AlreadyRunning { run_id: active.id });
            }
            match tx.test_runs().insert(&run).await {
                Ok(()) => {
                    tx.jobs().enqueue(&job).await?;
                    tx.commit().await?;
                    Ok(())
                }
                Err(error) => Err(error),
            }
        };
        match inserted {
            Ok(()) => Ok(run),
            Err(StoreError::Conflict(_)) => {
                // Lost a race with another start; the failed transaction is gone, so look afresh.
                let mut tx = self.store.begin().await?;
                match tx.test_runs().active_for(target).await? {
                    Some(active) => Err(AppError::AlreadyRunning { run_id: active.id }),
                    None => Err(AppError::Internal(
                        "a test run conflicted but none is active".to_owned(),
                    )),
                }
            }
            Err(other) => Err(other.into()),
        }
    }

    /// A run, for the screen that polls it.
    ///
    /// # Errors
    /// [`AppError::NotFound`].
    pub async fn get_test_run(
        &self,
        ctx: &RequestContext,
        id: TestRunId,
    ) -> Result<TestRun, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        tx.test_runs().get(id).await?.ok_or(AppError::NotFound)
    }

    /// The newest finished run for a target.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn latest_test(
        &self,
        ctx: &RequestContext,
        target: TestTarget,
    ) -> Result<Option<TestRun>, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        Ok(tx.test_runs().latest_finished(target).await?)
    }
}
