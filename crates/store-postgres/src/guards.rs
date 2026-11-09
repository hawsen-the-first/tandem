//! Sign-in guards (BR-3). Rows are locked `FOR UPDATE` so two simultaneous attempts cannot both
//! pass inside one delay (NFR design 5.2).

use crate::error::map_sqlx;
use crate::store::PgTx;
use async_trait::async_trait;
use tandem_core::ids::UserId;
use tandem_core::throttle::GuardState;
use tandem_core::token::TokenHash;
use tandem_ports::{GuardRepo, StoreError};
use time::OffsetDateTime;

/// The table of unknown-email guards never keeps more than this many rows (newest kept).
const UNKNOWN_GUARD_LIMIT: i64 = 10_000;

struct GuardRow {
    failure_count: i32,
    window_started_at: Option<OffsetDateTime>,
    delay_until: Option<OffsetDateTime>,
    locked_until: Option<OffsetDateTime>,
}

impl GuardRow {
    fn into_state(self) -> GuardState {
        GuardState {
            failure_count: u32::try_from(self.failure_count).unwrap_or(0),
            window_started_at: self.window_started_at,
            delay_until: self.delay_until,
            locked_until: self.locked_until,
        }
    }
}

fn stored_count(state: &GuardState) -> i32 {
    i32::try_from(state.failure_count).unwrap_or(i32::MAX)
}

impl PgTx {
    async fn evict_old_unknown_guards(&mut self) -> Result<(), StoreError> {
        sqlx::query!(
            "DELETE FROM unknown_sign_in_guards WHERE key_hash IN (
                 SELECT key_hash FROM unknown_sign_in_guards ORDER BY updated_at DESC, key_hash OFFSET $1)",
            UNKNOWN_GUARD_LIMIT
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl GuardRepo for PgTx {
    async fn get_account_guard_for_update(
        &mut self,
        user_id: UserId,
    ) -> Result<GuardState, StoreError> {
        sqlx::query!(
            "INSERT INTO sign_in_guards (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
            user_id.as_uuid()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        let row = sqlx::query_as!(
            GuardRow,
            "SELECT failure_count, window_started_at, delay_until, locked_until
             FROM sign_in_guards WHERE user_id = $1 FOR UPDATE",
            user_id.as_uuid()
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.into_state())
    }

    async fn put_account_guard(
        &mut self,
        user_id: UserId,
        state: &GuardState,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE sign_in_guards SET failure_count = $2, window_started_at = $3, delay_until = $4,
                    locked_until = $5 WHERE user_id = $1",
            user_id.as_uuid(),
            stored_count(state),
            state.window_started_at,
            state.delay_until,
            state.locked_until,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get_unknown_guard_for_update(
        &mut self,
        key: &TokenHash,
    ) -> Result<GuardState, StoreError> {
        sqlx::query!(
            "INSERT INTO unknown_sign_in_guards (key_hash, updated_at) VALUES ($1, now())
             ON CONFLICT (key_hash) DO NOTHING",
            key.as_str()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        self.evict_old_unknown_guards().await?;
        let row = sqlx::query_as!(
            GuardRow,
            "SELECT failure_count, window_started_at, delay_until, locked_until
             FROM unknown_sign_in_guards WHERE key_hash = $1 FOR UPDATE",
            key.as_str()
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.into_state())
    }

    async fn put_unknown_guard(
        &mut self,
        key: &TokenHash,
        state: &GuardState,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO unknown_sign_in_guards
                 (key_hash, failure_count, window_started_at, delay_until, locked_until, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (key_hash) DO UPDATE
             SET failure_count = EXCLUDED.failure_count, window_started_at = EXCLUDED.window_started_at,
                 delay_until = EXCLUDED.delay_until, locked_until = EXCLUDED.locked_until,
                 updated_at = EXCLUDED.updated_at",
            key.as_str(),
            stored_count(state),
            state.window_started_at,
            state.delay_until,
            state.locked_until,
            now,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        self.evict_old_unknown_guards().await
    }
}
