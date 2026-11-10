//! The commands themselves.

use crate::Output;
use crate::args::{
    AdminCommand, AuditCommand, Cli, Command, MigrateCommand, PruneArgs, VaultCommand,
};
use crate::error::CliError;
use crate::guards::{check_prune_cutoff, require_confirmation};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use tandem_app::AppError;
use tandem_config::{Config, secret_from_env};
use tandem_runtime::{Process, Services, load_vault};
use tandem_store_postgres::{
    PgStore, PoolConfig, RekeyError, applied_schema_version, connect_pool, expected_schema_version,
    migrate_down_one, migrate_up,
};
use time::OffsetDateTime;

const ADMIN_ROLE_URL: &str = "TANDEM_DATABASE_URL";
const PRIVILEGED_URL: &str = "TANDEM_PRIVILEGED_DATABASE_URL";
const PRUNE_STATEMENT_TIMEOUT: Duration = Duration::from_secs(600);

/// Run a parsed command against `environment` (the `TANDEM_*` variables).
///
/// # Errors
/// [`CliError`] with a one-line, secret-free message.
pub async fn run(
    cli: Cli,
    environment: &BTreeMap<String, String>,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    match cli.command {
        Command::Migrate(MigrateCommand::Up) => migrate_up_command(environment, out).await,
        Command::Migrate(MigrateCommand::Down(confirm)) => {
            require_confirmation(
                confirm.yes,
                "migrate down reverts the latest migration and drops its tables' data",
            )?;
            migrate_down_command(environment, out).await
        }
        Command::Vault(VaultCommand::Rekey) => rekey_command(environment, out).await,
        Command::Admin(AdminCommand::IssueReset(args)) => {
            issue_reset_command(environment, &args.email, out).await
        }
        Command::Audit(AuditCommand::Prune(args)) => prune_command(environment, args, out).await,
    }
}

fn database_error(error: impl std::fmt::Display) -> CliError {
    CliError::Database(error.to_string())
}

/// A pool for the role whose URL is in `url_name` (a secret: the variable or its `_FILE` form).
async fn pool_from_env(
    environment: &BTreeMap<String, String>,
    url_name: &'static str,
    statement_timeout: Duration,
) -> Result<sqlx::PgPool, CliError> {
    let url = secret_from_env(environment, url_name)?.ok_or(CliError::Missing(url_name))?;
    let ca_file = environment
        .get("TANDEM_DATABASE_CA_FILE")
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .ok_or(CliError::Missing("TANDEM_DATABASE_CA_FILE"))?;
    connect_pool(&PoolConfig {
        url: url.expose().to_owned(),
        ca_file,
        max_connections: 2,
        acquire_timeout: Duration::from_secs(10),
        statement_timeout,
        application_name: Process::Cli.name().to_owned(),
    })
    .await
    .map_err(database_error)
}

async fn migrate_up_command(
    environment: &BTreeMap<String, String>,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    let pool = pool_from_env(environment, ADMIN_ROLE_URL, Duration::from_secs(300)).await?;
    let before = applied_schema_version(&pool).await.unwrap_or(0);
    // sqlx holds a PostgreSQL advisory lock for the whole run, so two migrations cannot overlap.
    migrate_up(&pool)
        .await
        .map_err(|error| CliError::Migration(error.to_string()))?;
    let after = applied_schema_version(&pool)
        .await
        .map_err(database_error)?;
    out.line(&format!(
        "schema is at version {after} (was {before}; this build expects {})",
        expected_schema_version()
    ));
    pool.close().await;
    Ok(())
}

async fn migrate_down_command(
    environment: &BTreeMap<String, String>,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    let pool = pool_from_env(environment, ADMIN_ROLE_URL, Duration::from_secs(300)).await?;
    let reverted = migrate_down_one(&pool)
        .await
        .map_err(|error| CliError::Migration(error.to_string()))?;
    match reverted {
        Some(version) => out.line(&format!("reverted migration {version}")),
        None => out.line("no migration to revert"),
    }
    pool.close().await;
    Ok(())
}

async fn rekey_command(
    environment: &BTreeMap<String, String>,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    let config = Config::load(environment)?;
    let vault = load_vault(&config)?;
    let pool = pool_from_env(environment, ADMIN_ROLE_URL, Duration::from_secs(300)).await?;
    let store = PgStore::new(pool.clone());
    let report = store
        .rekey_sealed_values(|purpose, row_id, sealed| {
            vault
                .rekey(purpose, row_id, sealed)
                .map_err(|_| {
                    format!(
                        "a {purpose} value is sealed under a key this key file does not hold; list the old key as decrypt-only"
                    )
                })
        })
        .await
        .map_err(|error| match error {
            RekeyError::Value(message) => CliError::Vault(message),
            RekeyError::Store(error) => database_error(error),
        })?;
    out.line(&format!(
        "re-sealed {} MFA secret(s){}; {} value(s) were already under the current key (current key: {})",
        report.mfa_secrets,
        if report.canary { " and the vault canary" } else { "" },
        report.already_current,
        vault.current_key_id()
    ));
    pool.close().await;
    Ok(())
}

async fn issue_reset_command(
    environment: &BTreeMap<String, String>,
    email: &str,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    let config = Config::load(environment)?;
    let public_origin = config.public_url.origin().ascii_serialization();
    let services = Services::start(config, Process::Cli).await?;
    let link = services
        .app
        .issue_admin_reset_for_cli(email)
        .await
        .map_err(|error| match error {
            AppError::NotFound => CliError::Refused("no active admin has that email".to_owned()),
            AppError::Validation(issues) => CliError::Refused(issues.first().map_or_else(
                || "the request is not valid".to_owned(),
                |issue| issue.message.clone(),
            )),
            other => CliError::Refused(other.to_string()),
        })?;
    out.line(&format!(
        "Reset link for {} (also resets MFA; valid until {}):",
        link.user_display_name, link.expires_at
    ));
    out.line(&format!(
        "{public_origin}/reset-password?token={}",
        link.token
    ));
    services.store.pool().close().await;
    Ok(())
}

async fn prune_command(
    environment: &BTreeMap<String, String>,
    args: PruneArgs,
    out: &mut dyn Output,
) -> Result<(), CliError> {
    check_prune_cutoff(args.before, OffsetDateTime::now_utc())?;
    require_confirmation(
        args.yes,
        &format!(
            "audit prune permanently deletes entries before {}",
            args.before
        ),
    )?;
    let pool = pool_from_env(environment, PRIVILEGED_URL, PRUNE_STATEMENT_TIMEOUT).await?;
    let deleted = PgStore::new(pool.clone())
        .prune_audit_before(args.before)
        .await
        .map_err(database_error)?;
    out.line(&format!(
        "deleted {deleted} audit entr{} before {}",
        if deleted == 1 { "y" } else { "ies" },
        args.before
    ));
    pool.close().await;
    Ok(())
}
