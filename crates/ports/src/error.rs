//! Storage errors.

/// Boxed source error from an adapter.
pub type SourceError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Why a storage operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A required row does not exist.
    #[error("not found")]
    NotFound,
    /// A uniqueness rule was violated; the name identifies the constraint (for example
    /// `users.email`).
    #[error("conflict on {0}")]
    Conflict(&'static str),
    /// The store could not be reached in time.
    #[error("storage is unavailable")]
    Unavailable(#[source] SourceError),
    /// Any other storage failure.
    #[error("storage failure")]
    Other(#[source] SourceError),
}
