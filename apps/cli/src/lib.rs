//! The operations CLI: migrations, vault re-key, last-admin recovery and audit pruning.
//!
//! Every command returns a single-line error that never contains a secret; `main` turns it into a
//! non-zero exit status.

mod args;
mod commands;
mod error;
mod guards;

pub use args::{AdminCommand, AuditCommand, Cli, Command, MigrateCommand, VaultCommand};
pub use commands::run;
pub use error::CliError;
pub use guards::{MIN_AUDIT_RETENTION, check_prune_cutoff, parse_cutoff, require_confirmation};

/// Where command output goes (a trait so tests can capture it).
pub trait Output {
    /// Write one line.
    fn line(&mut self, text: &str);
}

/// Standard output.
#[derive(Debug, Default)]
pub struct Stdout;

impl Output for Stdout {
    #[allow(clippy::print_stdout)]
    fn line(&mut self, text: &str) {
        println!("{text}");
    }
}

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-cli";

#[cfg(test)]
mod tests;
