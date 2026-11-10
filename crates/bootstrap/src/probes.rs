//! Readiness probes shared by the api and the worker (LC-4): database, schema version, vault
//! canary.

use crate::bootstrap::VAULT_CANARY_KEY;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tandem_store_postgres::{PgStore, check_schema};
use tandem_telemetry::{ReadinessProbe, metric_names};
use tandem_vault::EnvelopeVault;

/// The database answers a trivial query (the caller bounds the time).
#[derive(Debug, Clone)]
pub struct DatabaseProbe {
    pool: PgPool,
}

impl DatabaseProbe {
    /// Probe `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReadinessProbe for DatabaseProbe {
    fn name(&self) -> &'static str {
        "database"
    }

    async fn check(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// The applied schema version is the one this binary expects (AV-3).
#[derive(Debug, Clone)]
pub struct SchemaProbe {
    pool: PgPool,
}

impl SchemaProbe {
    /// Probe `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReadinessProbe for SchemaProbe {
    fn name(&self) -> &'static str {
        "schema"
    }

    async fn check(&self) -> Result<(), String> {
        check_schema(&self.pool)
            .await
            .map_err(|error| error.to_string())
    }
}

/// The vault key still opens the stored canary.
#[derive(Debug, Clone)]
pub struct VaultProbe {
    store: PgStore,
    vault: Arc<EnvelopeVault>,
}

impl VaultProbe {
    /// Probe the canary kept in `store` with `vault`.
    pub fn new(store: PgStore, vault: Arc<EnvelopeVault>) -> Self {
        Self { store, vault }
    }
}

#[async_trait]
impl ReadinessProbe for VaultProbe {
    fn name(&self) -> &'static str {
        "vault"
    }

    async fn check(&self) -> Result<(), String> {
        let stored = self
            .store
            .internal_setting(VAULT_CANARY_KEY)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "the vault canary is missing".to_owned())?;
        let sealed = data_encoding::HEXLOWER
            .decode(stored.as_bytes())
            .map_err(|_| "the vault canary is not valid hex".to_owned())?;
        self.vault
            .verify_canary(&sealed)
            .map_err(|error| error.to_string())
    }
}

/// Publish `db_pool_connections{state}` every few seconds until shutdown.
pub fn spawn_pool_metrics(pool: PgPool, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    let size = f64::from(pool.size());
                    let idle = pool.num_idle() as f64;
                    metrics_gauge("size", size);
                    metrics_gauge("idle", idle);
                    metrics_gauge("in_use", (size - idle).max(0.0));
                }
                _ = shutdown.changed() => return,
            }
        }
    });
}

fn metrics_gauge(state: &'static str, value: f64) {
    tandem_telemetry::metrics::set_gauge_with_state(
        metric_names::DB_POOL_CONNECTIONS,
        state,
        value,
    );
}
