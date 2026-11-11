//! Job runner, outbox relay and cleanup (worker service).

mod health;
mod loops;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tandem_bootstrap::{
    DatabaseProbe, Process, SchemaProbe, Services, Shutdown, StartupError, VaultProbe,
    init_observability, spawn_pool_metrics, wait_for_termination,
};
use tandem_config::{Config, Secret};
use tandem_support::{Backoff, supervise};
use tandem_telemetry::{MetricsHandle, Readiness, ReadinessProbe};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub use health::{HealthState, MAX_RELAY_LAG, RelayLagProbe, constant_time_eq, router};
pub use loops::{RELAY_BATCH, cleanup_task, job_task, relay_task, stats_task};

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-worker";

/// Timings and addresses; tests shorten the intervals.
#[derive(Debug, Clone)]
pub struct WorkerOptions {
    /// Where the health and metrics endpoints listen.
    pub listen_addr: SocketAddr,
    /// Bearer token for `/metrics`.
    pub metrics_token: Option<Secret>,
    /// Relay fallback poll and idle job poll.
    pub poll: Duration,
    /// How often queue figures are published.
    pub stats_interval: Duration,
    /// Delay before the first cleanup.
    pub cleanup_first_delay: Duration,
    /// Time between cleanups (daily).
    pub cleanup_interval: Duration,
    /// How often lapsed job leases are recovered.
    pub recover_interval: Duration,
    /// How long shutdown waits for work in flight.
    pub drain_timeout: Duration,
}

impl WorkerOptions {
    /// The production timings for `config`.
    pub fn from_config(config: &Config) -> Self {
        Self {
            listen_addr: config.worker_listen_addr,
            metrics_token: config.metrics_token.clone(),
            poll: Duration::from_secs(1),
            stats_interval: Duration::from_secs(5),
            cleanup_first_delay: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(24 * 60 * 60),
            recover_interval: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(30),
        }
    }
}

const LEASE_TTL: time::Duration = time::Duration::seconds(60);
const TASK_BACKOFF: Backoff = Backoff::new(time::Duration::seconds(1), time::Duration::seconds(30));

/// A running worker.
#[derive(Debug)]
pub struct WorkerHandle {
    shutdown: Shutdown,
    server_shutdown: Shutdown,
    readiness: Readiness,
    in_flight: Arc<RwLock<()>>,
    tasks: Vec<JoinHandle<()>>,
    server: JoinHandle<()>,
    local_addr: SocketAddr,
    drain_timeout: Duration,
}

impl WorkerHandle {
    /// The address the health server bound.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Readiness turns false at once and the loops are told to stop; the health server keeps
    /// answering (503) until [`Self::stop`].
    pub fn begin_shutdown(&self) {
        self.readiness.begin_shutdown();
        self.shutdown.trigger();
    }

    /// Finish: wait for work in flight (bounded), let the loops end, stop the health server.
    pub async fn stop(self) {
        self.begin_shutdown();
        if tokio::time::timeout(self.drain_timeout, self.in_flight.write())
            .await
            .is_err()
        {
            tracing::warn!("work was still running after the drain timeout; its lease will lapse");
        }
        for task in self.tasks {
            if tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .is_err()
            {
                tracing::warn!("a worker loop did not stop in time");
            }
        }
        self.server_shutdown.trigger();
        let _ = tokio::time::timeout(Duration::from_secs(5), self.server).await;
    }
}

/// Start the loops and the health server over already-built services.
///
/// # Errors
/// An I/O error when the health address cannot be bound.
pub async fn start(
    services: &Services,
    metrics: MetricsHandle,
    options: WorkerOptions,
) -> std::io::Result<WorkerHandle> {
    let shutdown = Shutdown::new();
    let server_shutdown = Shutdown::new();
    let in_flight: Arc<RwLock<()>> = Arc::new(RwLock::new(()));
    let pool = services.store.pool().clone();
    spawn_pool_metrics(pool.clone(), shutdown.subscribe());

    let probes: Vec<Arc<dyn ReadinessProbe>> = vec![
        Arc::new(DatabaseProbe::new(pool.clone())),
        Arc::new(SchemaProbe::new(pool)),
        Arc::new(VaultProbe::new(
            services.store.clone(),
            services.vault.clone(),
        )),
        Arc::new(RelayLagProbe::new(services.store.clone(), MAX_RELAY_LAG)),
    ];
    let readiness = Readiness::new(probes, Duration::from_secs(1));

    let mut tasks = Vec::new();
    {
        let (app, store, flight, poll) = (
            services.app.clone(),
            services.store.clone(),
            Arc::clone(&in_flight),
            options.poll,
        );
        let receiver = shutdown.subscribe();
        tasks.push(tokio::spawn(supervise(
            "outbox_relay",
            TASK_BACKOFF,
            shutdown.subscribe(),
            move || {
                relay_task(
                    app.clone(),
                    store.clone(),
                    Arc::clone(&flight),
                    receiver.clone(),
                    poll,
                )
            },
        )));
    }
    {
        let (app, flight, poll, recover) = (
            services.app.clone(),
            Arc::clone(&in_flight),
            options.poll,
            options.recover_interval,
        );
        let receiver = shutdown.subscribe();
        tasks.push(tokio::spawn(supervise(
            "job_runner",
            TASK_BACKOFF,
            shutdown.subscribe(),
            move || {
                job_task(
                    app.clone(),
                    Arc::clone(&flight),
                    receiver.clone(),
                    poll,
                    LEASE_TTL,
                    recover,
                )
            },
        )));
    }
    {
        let (store, flight, first, every) = (
            services.store.clone(),
            Arc::clone(&in_flight),
            options.cleanup_first_delay,
            options.cleanup_interval,
        );
        let receiver = shutdown.subscribe();
        tasks.push(tokio::spawn(supervise(
            "cleanup",
            TASK_BACKOFF,
            shutdown.subscribe(),
            move || {
                cleanup_task(
                    store.clone(),
                    Arc::clone(&flight),
                    receiver.clone(),
                    first,
                    every,
                )
            },
        )));
    }
    {
        let (store, every) = (services.store.clone(), options.stats_interval);
        let receiver = shutdown.subscribe();
        tasks.push(tokio::spawn(supervise(
            "queue_stats",
            TASK_BACKOFF,
            shutdown.subscribe(),
            move || stats_task(store.clone(), receiver.clone(), every),
        )));
    }

    let listener = tokio::net::TcpListener::bind(options.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    let app = router(HealthState {
        readiness: readiness.clone(),
        metrics,
        metrics_token: options.metrics_token,
    });
    let mut stop_server = server_shutdown.subscribe();
    let server = tokio::spawn(async move {
        let served = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = stop_server.changed().await;
            })
            .await;
        if let Err(error) = served {
            tracing::error!(%error, "the health server stopped");
        }
    });
    tracing::info!(%local_addr, "worker started");
    Ok(WorkerHandle {
        shutdown,
        server_shutdown,
        readiness,
        in_flight,
        tasks,
        server,
        local_addr,
        drain_timeout: options.drain_timeout,
    })
}

/// Run the worker until SIGTERM or SIGINT.
///
/// # Errors
/// [`StartupError`] when a start-up guard fails; the process must exit non-zero.
pub async fn run(config: Config) -> Result<(), StartupError> {
    let metrics = init_observability(&config)?;
    let options = WorkerOptions::from_config(&config);
    let services = Services::start(config, Process::Worker).await?;
    let handle = start(&services, metrics, options).await.map_err(|error| {
        StartupError::Internal(format!("cannot start the health server: {error}"))
    })?;
    wait_for_termination().await;
    tracing::info!("shutting down");
    handle.stop().await;
    services.store.pool().close().await;
    Ok(())
}

#[cfg(test)]
mod tests;
