//! Traits for everything outside the domain: storage, credentials, vault and model providers.
//! Application services depend on these; adapter crates implement them.

pub mod config_repos;
pub mod credentials;
pub mod error;
pub mod model;
pub mod probe;
pub mod repos;
pub mod store;

pub use config_repos::{
    ConnectorRepo, DefaultsRepo, LockGuard, Locks, PolicyRepo, ProviderRepo, SearchRepo,
    SecretChange, TestRunRepo, capabilities_from,
};
pub use credentials::{
    BreachChecker, HashError, PasswordHasher, TotpService, Vault, VaultError, Verification,
};
pub use error::StoreError;
pub use model::{ModelProvider, ModelRequest, ModelResponse, ProviderError};
pub use probe::{
    ConnectorConnection, ConnectorProbe, ModelToProbe, ProviderConnection, ProviderProbe,
    ResultSink, RuntimeProbe, SearchConnection, SearchProbe,
};
pub use repos::{
    AuditCursor, AuditFilter, AuditPage, AuditRepo, ChatRepo, GuardRepo, InvitationRepo, JobQueue,
    MfaRepo, OutboxRepo, PasswordResetRepo, SessionRepo, SettingsRepo, UserRepo,
};
pub use store::{Store, Tx};

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-ports";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
