//! Mapping from SQLx errors to the storage errors the application understands.

use tandem_ports::StoreError;

/// Translate a SQLx error: uniqueness violations become [`StoreError::Conflict`] with a stable
/// constraint name; connection and timeout problems become [`StoreError::Unavailable`].
pub fn map_sqlx(error: sqlx::Error) -> StoreError {
    match &error {
        sqlx::Error::RowNotFound => StoreError::NotFound,
        sqlx::Error::Database(database) if database.is_unique_violation() => {
            StoreError::Conflict(match database.constraint() {
                Some("users_email_key") => "users.email",
                Some("chat_messages_in_reply_to_key") => "chat_messages.in_reply_to",
                Some("invitations_token_hash_key") => "invitations.token_hash",
                Some("password_resets_token_hash_key") => "password_resets.token_hash",
                Some("auth_sessions_token_hash_key") => "auth_sessions.token_hash",
                Some("recovery_codes_user_code_key") => "recovery_codes.user_code",
                _ => "unique_violation",
            })
        }
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => StoreError::Unavailable(Box::new(error)),
        _ => StoreError::Other(Box::new(error)),
    }
}

/// A stored value that cannot be turned back into a domain value.
pub(crate) fn corrupt(what: &str) -> StoreError {
    StoreError::Other(format!("stored {what} is invalid").into())
}
