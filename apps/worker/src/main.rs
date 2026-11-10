//! The worker process: outbox relay, job runner and cleanup.

use std::process::ExitCode;
use tandem_config::Config;

#[allow(clippy::print_stderr)]
fn report(message: &str) {
    eprintln!("{message}");
}

#[tokio::main]
async fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            report(&error.to_string());
            return ExitCode::FAILURE;
        }
    };
    match tandem_worker::run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&format!("tandem-worker: {error}"));
            ExitCode::FAILURE
        }
    }
}
