//! The `tandem-api` process.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tandem_api::routes::openapi;
use tandem_api::routes::ws::spawn_listener;
use tandem_api::server::{StateInputs, build, serve};
use tandem_config::Config;
use tandem_runtime::{
    DatabaseProbe, Process, SchemaProbe, Services, Shutdown, VaultProbe, init_observability,
    spawn_pool_metrics, wait_for_termination,
};
use tandem_telemetry::{Readiness, ReadinessProbe};
use tokio::net::TcpListener;

const SHUTDOWN_GRACE: Duration = Duration::from_secs(30);
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        return match openapi::document_json() {
            Ok(json) => {
                write_stdout(&json);
                ExitCode::SUCCESS
            }
            Err(error) => fail(&format!("could not render the OpenAPI document: {error}")),
        };
    }
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => fail(&message),
    }
}

#[allow(clippy::print_stdout)]
fn write_stdout(text: &str) {
    println!("{text}");
}

#[allow(clippy::print_stderr)]
fn fail(message: &str) -> ExitCode {
    eprintln!("tandem-api: {message}");
    ExitCode::FAILURE
}

async fn run() -> Result<(), String> {
    let config = Config::from_env().map_err(|error| error.to_string())?;
    let metrics = init_observability(&config).map_err(|error| error.to_string())?;
    let services = Services::start(config, Process::Api)
        .await
        .map_err(|error| error.to_string())?;
    let shutdown = Shutdown::new();
    let pool = services.store.pool().clone();

    let probes: Vec<Arc<dyn ReadinessProbe>> = vec![
        Arc::new(DatabaseProbe::new(pool.clone())),
        Arc::new(SchemaProbe::new(pool.clone())),
        Arc::new(VaultProbe::new(
            services.store.clone(),
            services.vault.clone(),
        )),
    ];
    let readiness = Readiness::new(probes, PROBE_TIMEOUT);
    let (router, state) = build(StateInputs {
        app: services.app.clone(),
        config: services.config.clone(),
        readiness: readiness.clone(),
        metrics,
        shutdown: shutdown.clone(),
        web_dir: services.config.web_dir.clone(),
    })
    .map_err(|error| format!("route table: {error}"))?;

    spawn_pool_metrics(pool.clone(), shutdown.subscribe());
    spawn_listener(pool.clone(), state.hub.clone(), shutdown.subscribe());

    let listener = TcpListener::bind(services.config.listen_addr)
        .await
        .map_err(|error| format!("cannot listen on {}: {error}", services.config.listen_addr))?;
    tracing::info!(address = %services.config.listen_addr, "api listening");

    let server = tokio::spawn(serve(listener, router, shutdown.clone()));
    wait_for_termination().await;
    tracing::info!("shutting down");
    readiness.begin_shutdown();
    shutdown.trigger();
    match tokio::time::timeout(SHUTDOWN_GRACE, server).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => tracing::error!(%error, "server stopped with an error"),
        Ok(Err(error)) => tracing::error!(%error, "server task failed"),
        Err(_) => tracing::warn!("in-flight requests did not finish within the grace period"),
    }
    pool.close().await;
    Ok(())
}
