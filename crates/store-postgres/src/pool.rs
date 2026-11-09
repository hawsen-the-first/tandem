//! Connection pools with mandatory, verified TLS (SE-1) and explicit timeouts (RL-1).

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

/// Settings for one pool. The API and the worker each build their own (bulkheads, SC-4).
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// `postgres://user:password@host:port/db?sslmode=verify-full`
    pub url: String,
    /// The CA certificate that signed the server certificate.
    pub ca_file: PathBuf,
    /// Upper bound on connections.
    pub max_connections: u32,
    /// How long to wait for a free connection.
    pub acquire_timeout: Duration,
    /// Server-side limit per statement.
    pub statement_timeout: Duration,
    /// Shown in `pg_stat_activity`.
    pub application_name: String,
}

/// Why a pool could not be created.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    /// The URL is not a valid PostgreSQL URL.
    #[error("the database URL is not valid: {0}")]
    InvalidUrl(String),
    /// The URL does not demand verified TLS (SECURITY-01).
    #[error("the database URL must use sslmode=verify-full or sslmode=verify-ca")]
    InsecureTransport,
    /// The connection failed.
    #[error("could not connect to the database: {0}")]
    Connect(#[source] sqlx::Error),
}

/// Build connection options, refusing anything but verified TLS.
///
/// # Errors
/// [`PoolError::InvalidUrl`] or [`PoolError::InsecureTransport`].
pub fn secure_options(config: &PoolConfig) -> Result<PgConnectOptions, PoolError> {
    let parsed =
        url::Url::parse(&config.url).map_err(|error| PoolError::InvalidUrl(error.to_string()))?;
    let mode = parsed
        .query_pairs()
        .find(|(key, _)| key == "sslmode")
        .map(|(_, value)| value.into_owned());
    let ssl_mode = match mode.as_deref() {
        Some("verify-full") => PgSslMode::VerifyFull,
        Some("verify-ca") => PgSslMode::VerifyCa,
        _ => return Err(PoolError::InsecureTransport),
    };
    let statement_timeout_ms = config.statement_timeout.as_millis().to_string();
    Ok(PgConnectOptions::from_str(&config.url)
        .map_err(|error| PoolError::InvalidUrl(error.to_string()))?
        .ssl_mode(ssl_mode)
        .ssl_root_cert(&config.ca_file)
        .application_name(&config.application_name)
        .options([("statement_timeout", statement_timeout_ms.as_str())]))
}

/// Create a pool and prove it can connect.
///
/// # Errors
/// [`PoolError`] when the URL is insecure or invalid, or the database is unreachable.
pub async fn connect_pool(config: &PoolConfig) -> Result<PgPool, PoolError> {
    let options = secure_options(config)?;
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .connect_with(options)
        .await
        .map_err(PoolError::Connect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(url: &str) -> PoolConfig {
        PoolConfig {
            url: url.to_owned(),
            ca_file: PathBuf::from("/nonexistent/ca.crt"),
            max_connections: 5,
            acquire_timeout: Duration::from_secs(2),
            statement_timeout: Duration::from_secs(5),
            application_name: "test".to_owned(),
        }
    }

    #[test]
    fn se_1_urls_without_verified_tls_are_refused() {
        for url in [
            "postgres://u:p@h:5432/db",
            "postgres://u:p@h:5432/db?sslmode=disable",
            "postgres://u:p@h:5432/db?sslmode=prefer",
            "postgres://u:p@h:5432/db?sslmode=require",
            "postgres://u:p@h:5432/db?sslmode=allow",
        ] {
            assert!(
                matches!(
                    secure_options(&config(url)),
                    Err(PoolError::InsecureTransport)
                ),
                "{url}"
            );
        }
    }

    #[test]
    fn se_1_verify_full_and_verify_ca_are_accepted() {
        for url in [
            "postgres://u:p@h:5432/db?sslmode=verify-full",
            "postgres://u:p@h:5432/db?sslmode=verify-ca",
        ] {
            assert!(secure_options(&config(url)).is_ok(), "{url}");
        }
    }

    #[test]
    fn malformed_urls_are_reported_without_echoing_secrets() {
        let error = secure_options(&config("not a url")).unwrap_err();
        assert!(matches!(error, PoolError::InvalidUrl(_)));
    }
}
