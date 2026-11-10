//! Command-line arguments.

use crate::guards::parse_cutoff;
use clap::{Args, Parser, Subcommand};
use time::OffsetDateTime;

/// Tandem operations: run beside the api and worker, with the same environment.
#[derive(Debug, Parser)]
#[command(name = "tandem", version, about)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Apply or revert database migrations (needs the migration role in TANDEM_DATABASE_URL).
    #[command(subcommand)]
    Migrate(MigrateCommand),
    /// Vault key operations.
    #[command(subcommand)]
    Vault(VaultCommand),
    /// Administrator recovery.
    #[command(subcommand)]
    Admin(AdminCommand),
    /// Audit log maintenance.
    #[command(subcommand)]
    Audit(AuditCommand),
}

/// `tandem migrate ...`
#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    /// Apply every pending migration.
    Up,
    /// Revert the most recent migration (data in its tables is lost).
    Down(ConfirmArgs),
}

/// `tandem vault ...`
#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// Re-seal every sealed value under the current key of the key file. The key file must also
    /// list every older key still in use as `decrypt-only`.
    Rekey,
}

/// `tandem admin ...`
#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    /// Print a one-time password and MFA reset link for the only active admin.
    IssueReset(IssueResetArgs),
}

/// `tandem audit ...`
#[derive(Debug, Subcommand)]
pub enum AuditCommand {
    /// Delete audit entries older than a date. The application role cannot do this: it needs a
    /// privileged credential in TANDEM_PRIVILEGED_DATABASE_URL (or its _FILE form).
    Prune(PruneArgs),
}

/// A destructive command's confirmation.
#[derive(Debug, Args)]
pub struct ConfirmArgs {
    /// Confirm that you mean it.
    #[arg(long)]
    pub yes: bool,
}

/// Arguments of `admin issue-reset`.
#[derive(Debug, Args)]
pub struct IssueResetArgs {
    /// The admin's email address.
    #[arg(long)]
    pub email: String,
}

/// Arguments of `audit prune`.
#[derive(Debug, Args)]
pub struct PruneArgs {
    /// Delete entries older than this (RFC 3339 or YYYY-MM-DD); at least 90 days ago.
    #[arg(long, value_parser = parse_cutoff)]
    pub before: OffsetDateTime,
    /// Confirm that you mean it.
    #[arg(long)]
    pub yes: bool,
}
