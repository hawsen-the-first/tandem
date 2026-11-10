//! `tandem`: the operations command line.

use clap::Parser;
use std::collections::BTreeMap;
use std::process::ExitCode;
use tandem_cli::{Cli, Stdout, run};

#[allow(clippy::print_stderr)]
fn report(message: &str) {
    eprintln!("{message}");
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let environment: BTreeMap<String, String> = std::env::vars()
        .filter(|(name, _)| name.starts_with("TANDEM_"))
        .collect();
    match run(cli, &environment, &mut Stdout).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&format!("tandem: {error}"));
            ExitCode::FAILURE
        }
    }
}
