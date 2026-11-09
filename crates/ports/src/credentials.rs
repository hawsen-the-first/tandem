//! Password hashing, breached-password checking, TOTP and the secret vault.

use async_trait::async_trait;
use tandem_support::RngError;
use time::OffsetDateTime;

/// Why hashing failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HashError {
    /// Too many hashes are running; the caller should answer "busy, try again" (NFR design 4).
    #[error("the system is busy; try again")]
    Busy,
    /// Anything else.
    #[error("password hashing failed: {0}")]
    Internal(String),
}

/// The result of checking a password against a stored hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verification {
    /// The password matches.
    pub matches: bool,
    /// The stored hash uses outdated parameters and should be replaced.
    pub needs_rehash: bool,
}

/// Argon2id hashing with bounded concurrency.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    /// Hash a password with the current profile.
    async fn hash(&self, password: &str) -> Result<String, HashError>;
    /// Check a password against a stored hash.
    async fn verify(&self, password: &str, stored: &str) -> Result<Verification, HashError>;
    /// Do the same work as [`Self::verify`] against a dummy hash (unknown accounts, BR-3.5).
    async fn dummy_verify(&self) -> Result<(), HashError>;
}

/// The breached-password check (BR-2.3). Degradation (online service down) is handled inside.
#[async_trait]
pub trait BreachChecker: Send + Sync {
    /// Whether the password appears in breach data. `use_online` adds the online check.
    async fn is_breached(&self, password: &str, use_online: bool) -> bool;
}

/// TOTP (RFC 6238): 6 digits, 30-second steps, one step of tolerance either side (BR-4.3).
pub trait TotpService: Send + Sync {
    /// A new random secret.
    ///
    /// # Errors
    /// [`RngError`] if the random source fails.
    fn generate_secret(&self) -> Result<Vec<u8>, RngError>;
    /// The secret as base32 text for manual entry.
    fn secret_base32(&self, secret: &[u8]) -> String;
    /// The `otpauth://` URI encoded in the QR code.
    fn otpauth_uri(&self, secret: &[u8], account: &str, issuer: &str) -> String;
    /// Check a code at `at`; returns the matching time-step if valid.
    fn verify(&self, secret: &[u8], code: &str, at: OffsetDateTime) -> Option<u64>;
}

/// Why a vault operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VaultError {
    /// Sealing failed.
    #[error("sealing failed")]
    Seal,
    /// Opening failed (wrong key, wrong context or tampering).
    #[error("opening failed")]
    Open,
}

/// Envelope encryption for secrets at rest (SE-2). `purpose` and `row_id` bind a sealed value to
/// where it is stored.
pub trait Vault: Send + Sync {
    /// Seal a secret.
    fn seal(&self, purpose: &str, row_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, VaultError>;
    /// Open a sealed secret.
    fn open(&self, purpose: &str, row_id: &str, sealed: &[u8]) -> Result<Vec<u8>, VaultError>;
}
