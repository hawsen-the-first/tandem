//! CLI errors. Messages are single lines and carry no secret values.

use tandem_config::ConfigError;
use tandem_runtime::StartupError;

/// Why a command failed.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// A destructive command was run without `--yes`.
    #[error("{0}")]
    NotConfirmed(String),
    /// An argument value was refused.
    #[error("{0}")]
    BadArgument(String),
    /// A required setting is missing.
    #[error("{0} is required")]
    Missing(&'static str),
    /// Configuration was invalid (one line per problem, joined).
    #[error("{0}")]
    Config(String),
    /// The database refused or could not be reached.
    #[error("database: {0}")]
    Database(String),
    /// A migration failed.
    #[error("migration: {0}")]
    Migration(String),
    /// The vault could not re-seal.
    #[error("vault: {0}")]
    Vault(String),
    /// The application refused the request.
    #[error("{0}")]
    Refused(String),
    /// Start-up of the application services failed.
    #[error("{0}")]
    Startup(String),
}

impl From<ConfigError> for CliError {
    fn from(error: ConfigError) -> Self {
        let joined = error
            .problems
            .iter()
            .map(|problem| format!("{}: {}", problem.setting, problem.message))
            .collect::<Vec<_>>()
            .join("; ");
        Self::Config(joined)
    }
}

impl From<StartupError> for CliError {
    fn from(error: StartupError) -> Self {
        Self::Startup(error.to_string())
    }
}
