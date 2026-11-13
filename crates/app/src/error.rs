//! Errors returned by application services. The API layer maps each to an HTTP status and a
//! user-safe message; internal details never reach users (SECURITY-15).

use tandem_ports::{HashError, StoreError, VaultError};
use tandem_support::RngError;
use time::OffsetDateTime;

/// One problem with one input field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldIssue {
    /// The field name as the client knows it.
    pub field: &'static str,
    /// What to fix, in plain words.
    pub message: String,
}

/// Why a use case failed.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The thing does not exist, or the caller may not know it exists (BR-1.1, BR-9.2).
    #[error("not found")]
    NotFound,
    /// The caller is signed in but may not do this.
    #[error("forbidden")]
    Forbidden,
    /// The caller is not signed in (or the sign-in is no longer valid).
    #[error("not signed in")]
    Unauthenticated,
    /// Input the user can fix.
    #[error("the input is not valid")]
    Validation(Vec<FieldIssue>),
    /// Wrong email, password, code or locked account; one generic answer (BR-3.5).
    #[error("those details do not match")]
    InvalidCredentials,
    /// Too many attempts; try again at the given time (BR-3.6).
    #[error("too many attempts")]
    Refused {
        /// When the next attempt will be accepted.
        retry_at: OffsetDateTime,
    },
    /// The host is busy hashing passwords; try again shortly (NFR design 4).
    #[error("the system is busy")]
    Busy,
    /// An unknown, expired, revoked or used link (BR-5.4, BR-6.2).
    #[error("this link is not valid")]
    InvalidLink,
    /// A test is already queued or running for this target; the screen attaches to it.
    #[error("a test is already running")]
    AlreadyRunning {
        /// The run in progress.
        run_id: tandem_core::ids::TestRunId,
    },
    /// Anything unexpected. The message is for logs only.
    #[error("internal error")]
    Internal(String),
}

impl AppError {
    /// A validation error about a single field.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation(vec![FieldIssue {
            field,
            message: message.into(),
        }])
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        Self::Internal(format!("storage: {error}"))
    }
}

impl From<RngError> for AppError {
    fn from(error: RngError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<VaultError> for AppError {
    fn from(error: VaultError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<HashError> for AppError {
    fn from(error: HashError) -> Self {
        match error {
            HashError::Busy => Self::Busy,
            HashError::Internal(message) => Self::Internal(message),
        }
    }
}
