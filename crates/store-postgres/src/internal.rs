//! Internal settings: values the application stores for itself (the vault canary, the hashing
//! profile). They live in `instance_settings` with no `updated_by`, are not part of the `ports`
//! settings repository and are never shown by the settings API.

use crate::error::map_sqlx;
use crate::store::PgStore;
use tandem_ports::StoreError;
use time::OffsetDateTime;

impl PgStore {
    /// Read one internal setting.
    ///
    /// # Errors
    /// [`StoreError`] when the query fails.
    pub async fn internal_setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        let row = sqlx::query!("SELECT value FROM instance_settings WHERE key = $1", key)
            .fetch_optional(self.pool())
            .await
            .map_err(map_sqlx)?;
        Ok(row.map(|row| row.value))
    }

    /// Store an internal setting only if none exists. Returns whether this call stored it, so two
    /// processes starting together agree on one value.
    ///
    /// # Errors
    /// [`StoreError`] when the query fails.
    pub async fn set_internal_setting_if_absent(
        &self,
        key: &str,
        value: &str,
        now: OffsetDateTime,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query!(
            "INSERT INTO instance_settings (key, value, updated_by, updated_at) VALUES ($1, $2, NULL, $3)
             ON CONFLICT (key) DO NOTHING",
            key,
            value,
            now,
        )
        .execute(self.pool())
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() == 1)
    }

    /// Store an internal setting, replacing any existing value.
    ///
    /// # Errors
    /// [`StoreError`] when the query fails.
    pub async fn set_internal_setting(
        &self,
        key: &str,
        value: &str,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO instance_settings (key, value, updated_by, updated_at) VALUES ($1, $2, NULL, $3)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
            key,
            value,
            now,
        )
        .execute(self.pool())
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

/// Why a re-key stopped (and rolled back).
#[derive(Debug, thiserror::Error)]
pub enum RekeyError {
    /// Storage failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A value could not be opened or re-sealed.
    #[error("{0}")]
    Value(String),
}

/// What a vault re-seal changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RekeyReport {
    /// Number of MFA credentials re-sealed (a count; no secret value).
    pub mfa_credentials_resealed: u64,
    /// Number of provider API keys re-sealed.
    pub provider_keys_resealed: u64,
    /// Number of search API keys re-sealed.
    pub search_keys_resealed: u64,
    /// Number of connector tokens re-sealed.
    pub connector_secrets_resealed: u64,
    /// Whether the vault canary was re-sealed.
    pub canary: bool,
    /// Values already sealed under the current key.
    pub already_current: u64,
}

impl PgStore {
    /// Re-seal every sealed value (MFA secrets, provider and search keys, connector tokens and the vault canary) with `rekey`, all in one
    /// transaction. `rekey(purpose, row_id, sealed)` returns the new sealed value, or `None` when
    /// it is already current. Any error (a value that cannot be opened, for example) rolls back
    /// everything, so a partial re-key never happens.
    ///
    /// # Errors
    /// [`StoreError`] for storage failures or the first message `rekey` returns.
    pub async fn rekey_sealed_values<F>(&self, mut rekey: F) -> Result<RekeyReport, RekeyError>
    where
        F: FnMut(&str, &str, &[u8]) -> Result<Option<Vec<u8>>, String> + Send,
    {
        let mut report = RekeyReport::default();
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let secrets = sqlx::query!("SELECT user_id, secret_sealed FROM mfa_credentials FOR UPDATE")
            .fetch_all(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        for row in secrets {
            let row_id = row.user_id.to_string();
            match rekey("mfa_secret", &row_id, &row.secret_sealed).map_err(RekeyError::Value)? {
                Some(sealed) => {
                    sqlx::query!(
                        "UPDATE mfa_credentials SET secret_sealed = $2 WHERE user_id = $1",
                        row.user_id,
                        sealed
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(map_sqlx)?;
                    report.mfa_credentials_resealed += 1;
                }
                None => report.already_current += 1,
            }
        }
        let providers = sqlx::query!(
            "SELECT id, api_key_sealed AS \"sealed!\" FROM providers WHERE api_key_sealed IS NOT NULL FOR UPDATE"
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        for row in providers {
            match rekey("provider_api_key", &row.id.to_string(), &row.sealed)
                .map_err(RekeyError::Value)?
            {
                Some(sealed) => {
                    sqlx::query!(
                        "UPDATE providers SET api_key_sealed = $2 WHERE id = $1",
                        row.id,
                        sealed
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(map_sqlx)?;
                    report.provider_keys_resealed += 1;
                }
                None => report.already_current += 1,
            }
        }
        let searches = sqlx::query!(
            "SELECT kind, api_key_sealed AS \"sealed!\" FROM search_configs WHERE api_key_sealed IS NOT NULL FOR UPDATE"
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        for row in searches {
            match rekey("search_api_key", &row.kind, &row.sealed).map_err(RekeyError::Value)? {
                Some(sealed) => {
                    sqlx::query!(
                        "UPDATE search_configs SET api_key_sealed = $2 WHERE kind = $1",
                        row.kind,
                        sealed
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(map_sqlx)?;
                    report.search_keys_resealed += 1;
                }
                None => report.already_current += 1,
            }
        }
        let connectors = sqlx::query!("SELECT kind, secret_sealed FROM connectors FOR UPDATE")
            .fetch_all(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        for row in connectors {
            match rekey("connector_secret", &row.kind, &row.secret_sealed)
                .map_err(RekeyError::Value)?
            {
                Some(sealed) => {
                    sqlx::query!(
                        "UPDATE connectors SET secret_sealed = $2 WHERE kind = $1",
                        row.kind,
                        sealed
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(map_sqlx)?;
                    report.connector_secrets_resealed += 1;
                }
                None => report.already_current += 1,
            }
        }
        let canary = sqlx::query!(
            "SELECT value FROM instance_settings WHERE key = 'internal.vault_canary' FOR UPDATE"
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if let Some(canary) = canary {
            let sealed = data_encoding::HEXLOWER
                .decode(canary.value.as_bytes())
                .map_err(|_| {
                    RekeyError::Value("the stored vault canary is not valid".to_owned())
                })?;
            match rekey("vault_canary", "canary", &sealed).map_err(RekeyError::Value)? {
                Some(resealed) => {
                    sqlx::query!(
                        "UPDATE instance_settings SET value = $1 WHERE key = 'internal.vault_canary'",
                        data_encoding::HEXLOWER.encode(&resealed)
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(map_sqlx)?;
                    report.canary = true;
                }
                None => report.already_current += 1,
            }
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(report)
    }

    /// Delete audit entries older than `cutoff`. The application role cannot do this (it has
    /// INSERT and SELECT only): call it on a pool connected with a privileged role.
    ///
    /// # Errors
    /// [`StoreError`] when the statement fails, including a permission error for the app role.
    pub async fn prune_audit_before(&self, cutoff: OffsetDateTime) -> Result<u64, StoreError> {
        let result = sqlx::query!(
            "DELETE FROM tandem.audit_entries WHERE occurred_at < $1",
            cutoff
        )
        .execute(self.pool())
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }
}
