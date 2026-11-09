//! Instance settings.

use crate::error::map_sqlx;
use crate::store::PgTx;
use async_trait::async_trait;
use std::collections::BTreeMap;
use tandem_core::ids::UserId;
use tandem_ports::{SettingsRepo, StoreError};
use time::OffsetDateTime;

#[async_trait]
impl SettingsRepo for PgTx {
    async fn get_all(&mut self) -> Result<BTreeMap<String, String>, StoreError> {
        let rows = sqlx::query!("SELECT key, value FROM instance_settings")
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(|row| (row.key, row.value)).collect())
    }

    async fn put(
        &mut self,
        key: &str,
        value: &str,
        updated_by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO instance_settings (key, value, updated_by, updated_at) VALUES ($1, $2, $3, $4)
             ON CONFLICT (key) DO UPDATE
             SET value = EXCLUDED.value, updated_by = EXCLUDED.updated_by, updated_at = EXCLUDED.updated_at",
            key,
            value,
            updated_by.as_uuid(),
            now,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}
