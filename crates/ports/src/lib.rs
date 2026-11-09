//! Traits for everything outside the domain: storage, credentials, vault and model providers.
//! Application services depend on these; adapter crates implement them.

pub mod credentials;
pub mod error;
pub mod model;
pub mod repos;
pub mod store;

pub use credentials::{
    BreachChecker, HashError, PasswordHasher, TotpService, Vault, VaultError, Verification,
};
pub use error::StoreError;
pub use model::{ModelProvider, ModelRequest, ModelResponse, ProviderError};
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
