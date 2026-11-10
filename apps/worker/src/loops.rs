//! The worker's supervised loops: outbox relay (LC-16), job runner (LC-18) and cleanup (LC-20),
//! plus the figures they publish.

use std::sync::Arc;
use std::time::Duration;
use tandem_app::{App, JobOutcome};
use tandem_store_postgres::{PgStore, RelayLease};
use tandem_support::supervise::TaskError;
use tandem_telemetry::metric_names;
use time::OffsetDateTime;
use tokio::sync::{RwLock, watch};

/// How many outbox rows one relay batch handles.
pub const RELAY_BATCH: usize = 100;
const RELAY_CHANNEL: &str = "tandem_events";
const FOLLOWER_RETRY: Duration = Duration::from_secs(5);

/// Held (shared) while work is in flight; shutdown takes it exclusively to wait for that work.
pub type InFlight = Arc<RwLock<()>>;

/// Sleep for `duration`, or until shutdown begins.
pub async fn pause(shutdown: &mut watch::Receiver<bool>, duration: Duration) {
    tokio::select! {
        () = tokio::time::sleep(duration) => {}
        _ = shutdown.changed() => {}
    }
}

/// Run as the relay when no other worker is: take the advisory lock, then publish outbox rows in
/// order, waking on `NOTIFY` and polling as a fallback. A lost lock or database error returns to
/// the supervisor, which restarts the loop after a backoff.
pub async fn relay_task(
    app: App,
    store: PgStore,
    in_flight: InFlight,
    mut shutdown: watch::Receiver<bool>,
    poll: Duration,
) -> Result<(), TaskError> {
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        let Some(lease) = RelayLease::try_acquire(store.pool()).await? else {
            pause(&mut shutdown, FOLLOWER_RETRY).await;
            continue;
        };
        tracing::info!("this worker is the outbox relay");
        let result = lead(&app, &store, &in_flight, &mut shutdown, poll).await;
        if let Err(error) = lease.release().await {
            tracing::warn!(%error, "could not release the relay lock cleanly");
        }
        result?;
    }
}

async fn lead(
    app: &App,
    store: &PgStore,
    in_flight: &InFlight,
    shutdown: &mut watch::Receiver<bool>,
    poll: Duration,
) -> Result<(), TaskError> {
    let mut listener = sqlx::postgres::PgListener::connect_with(store.pool()).await?;
    listener.listen(RELAY_CHANNEL).await?;
    loop {
        loop {
            let _work = Arc::clone(in_flight).read_owned().await;
            if app.relay_outbox_batch(RELAY_BATCH).await? == 0 {
                break;
            }
        }
        if *shutdown.borrow() {
            return Ok(());
        }
        tokio::select! {
            _ = shutdown.changed() => return Ok(()),
            woke = tokio::time::timeout(poll, listener.recv()) => {
                if let Ok(Err(error)) = woke {
                    return Err(error.into());
                }
            }
        }
    }
}

/// Lease and run due jobs; recover jobs whose worker died.
pub async fn job_task(
    app: App,
    in_flight: InFlight,
    mut shutdown: watch::Receiver<bool>,
    idle_poll: Duration,
    lease_ttl: time::Duration,
    recover_every: Duration,
) -> Result<(), TaskError> {
    let mut last_recovery: Option<tokio::time::Instant> = None;
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        if last_recovery.is_none_or(|at| at.elapsed() >= recover_every) {
            let recovered = app.recover_lapsed_jobs().await?;
            if recovered > 0 {
                tracing::warn!(recovered, "re-queued jobs whose worker stopped");
            }
            last_recovery = Some(tokio::time::Instant::now());
        }
        let work = Arc::clone(&in_flight).read_owned().await;
        let started = tokio::time::Instant::now();
        let outcome = app.process_next_job(lease_ttl).await?;
        drop(work);
        let seconds = started.elapsed().as_secs_f64();
        match outcome {
            JobOutcome::NoJob => pause(&mut shutdown, idle_poll).await,
            JobOutcome::Completed { kind } => {
                metrics::histogram!(metric_names::JOB_DURATION_SECONDS, "kind" => kind)
                    .record(seconds);
                tracing::info!(kind, "job completed");
            }
            JobOutcome::Retrying { kind, run_at } => {
                metrics::histogram!(metric_names::JOB_DURATION_SECONDS, "kind" => kind)
                    .record(seconds);
                tracing::warn!(kind, %run_at, "job failed and will retry");
            }
            JobOutcome::Failed { kind } => {
                metrics::histogram!(metric_names::JOB_DURATION_SECONDS, "kind" => kind)
                    .record(seconds);
                tracing::error!(kind, "job failed for the last time");
            }
        }
    }
}

/// Delete rows past retention, once after `first_delay` and then every `interval`.
pub async fn cleanup_task(
    store: PgStore,
    in_flight: InFlight,
    mut shutdown: watch::Receiver<bool>,
    first_delay: Duration,
    interval: Duration,
) -> Result<(), TaskError> {
    pause(&mut shutdown, first_delay).await;
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        let work = Arc::clone(&in_flight).read_owned().await;
        let report = store.cleanup(OffsetDateTime::now_utc()).await?;
        drop(work);
        tracing::info!(
            sessions = report.sessions,
            resets = report.resets,
            invitations = report.invitations,
            jobs = report.jobs,
            outbox_events = report.outbox_events,
            unknown_guards = report.unknown_guards,
            "retention cleanup finished"
        );
        pause(&mut shutdown, interval).await;
    }
}

/// Publish queue and outbox gauges until shutdown. Failures are logged and retried next tick.
pub async fn stats_task(
    store: PgStore,
    mut shutdown: watch::Receiver<bool>,
    interval: Duration,
) -> Result<(), TaskError> {
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        let now = OffsetDateTime::now_utc();
        match (store.outbox_stats(now).await, store.job_stats(now).await) {
            (Ok(outbox), Ok(jobs)) => {
                metrics::gauge!(metric_names::OUTBOX_PENDING).set(outbox.pending as f64);
                metrics::gauge!(metric_names::OUTBOX_RELAY_LAG_SECONDS)
                    .set(outbox.oldest_age.map_or(0.0, |age| age.as_seconds_f64()));
                metrics::gauge!(metric_names::JOB_QUEUE_DEPTH).set(jobs.queued as f64);
                metrics::gauge!(metric_names::JOB_OLDEST_AGE_SECONDS)
                    .set(jobs.oldest_due_age.map_or(0.0, |age| age.as_seconds_f64()));
            }
            (Err(error), _) | (_, Err(error)) => {
                tracing::warn!(%error, "could not read queue figures");
            }
        }
        pause(&mut shutdown, interval).await;
    }
}
