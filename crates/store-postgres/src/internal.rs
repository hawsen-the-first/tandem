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
