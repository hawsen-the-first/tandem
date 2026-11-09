//! Retention cleanup (LC-20, decisions Q6 A and Q2 B). It removes ended sign-ins, used or expired
//! tokens and finished jobs after 30 days, and published outbox rows after 24 hours. **It never
//! touches the audit log.**

use crate::error::map_sqlx;
use crate::store::PgStore;
use tandem_ports::StoreError;
use time::{Duration, OffsetDateTime};

/// How long ended sign-ins, tokens and finished jobs are kept.
pub const TOKEN_AND_JOB_RETENTION: Duration = Duration::days(30);
/// How long published outbox rows are kept (the WebSocket replay window).
pub const OUTBOX_RETENTION: Duration = Duration::hours(24);
/// How long unknown-email guards are kept.
pub const UNKNOWN_GUARD_RETENTION: Duration = Duration::hours(1);

/// How many rows each cleanup removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CleanupReport {
    /// Ended sign-ins.
    pub sessions: u64,
    /// Used or expired reset links.
    pub resets: u64,
    /// Accepted, revoked or expired invitations.
    pub invitations: u64,
    /// Finished jobs.
    pub jobs: u64,
    /// Published outbox events.
    pub outbox_events: u64,
    /// Stale unknown-email guards.
    pub unknown_guards: u64,
}

impl PgStore {
    /// Delete rows past their retention period.
    ///
    /// # Errors
    /// [`StoreError`] when a statement fails.
    pub async fn cleanup(&self, now: OffsetDateTime) -> Result<CleanupReport, StoreError> {
        let tokens_before = now - TOKEN_AND_JOB_RETENTION;
        let outbox_before = now - OUTBOX_RETENTION;
        let guards_before = now - UNKNOWN_GUARD_RETENTION;
        let pool = self.pool();
        let sessions = sqlx::query!(
            "DELETE FROM auth_sessions WHERE ended_at < $1",
            tokens_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        let resets = sqlx::query!(
            "DELETE FROM password_resets WHERE (used_at IS NOT NULL AND used_at < $1) OR expires_at < $1",
            tokens_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        let invitations = sqlx::query!(
            "DELETE FROM invitations WHERE created_at < $1 AND (status <> 'pending' OR expires_at < $1)",
            tokens_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        let jobs = sqlx::query!(
            "DELETE FROM jobs WHERE status IN ('done', 'failed') AND run_at < $1",
            tokens_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        let outbox_events = sqlx::query!(
            "DELETE FROM outbox_events WHERE published_at < $1",
            outbox_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        let unknown_guards = sqlx::query!(
            "DELETE FROM unknown_sign_in_guards WHERE updated_at < $1",
            guards_before
        )
        .execute(pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        Ok(CleanupReport {
            sessions,
            resets,
            invitations,
            jobs,
            outbox_events,
            unknown_guards,
        })
    }
}
