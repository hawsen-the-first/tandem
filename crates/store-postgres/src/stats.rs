//! Queue and outbox figures for metrics and the worker's readiness check.

use crate::error::map_sqlx;
use crate::store::PgStore;
use tandem_ports::StoreError;
use time::{Duration, OffsetDateTime};

/// How far behind the outbox relay is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxStats {
    /// Events not yet published.
    pub pending: i64,
    /// Age of the oldest unpublished event, if any.
    pub oldest_age: Option<Duration>,
}

/// How full the job queue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobStats {
    /// Jobs waiting to run.
    pub queued: i64,
    /// How long the oldest due job has been waiting, if any is due.
    pub oldest_due_age: Option<Duration>,
}

impl PgStore {
    /// Unpublished outbox events and the age of the oldest.
    ///
    /// # Errors
    /// [`StoreError`] when the query fails.
    pub async fn outbox_stats(&self, now: OffsetDateTime) -> Result<OutboxStats, StoreError> {
        let row = sqlx::query!(
            r#"SELECT count(*) AS "pending!", min(created_at) AS oldest FROM outbox_events WHERE published_at IS NULL"#
        )
        .fetch_one(self.pool())
        .await
        .map_err(map_sqlx)?;
        Ok(OutboxStats {
            pending: row.pending,
            oldest_age: row.oldest.map(|oldest| (now - oldest).max(Duration::ZERO)),
        })
    }

    /// Queued jobs and how long the oldest due one has waited.
    ///
    /// # Errors
    /// [`StoreError`] when the query fails.
    pub async fn job_stats(&self, now: OffsetDateTime) -> Result<JobStats, StoreError> {
        let row = sqlx::query!(
            r#"SELECT count(*) AS "queued!", min(run_at) FILTER (WHERE run_at <= $1) AS oldest_due
               FROM jobs WHERE status = 'queued'"#,
            now
        )
        .fetch_one(self.pool())
        .await
        .map_err(map_sqlx)?;
        Ok(JobStats {
            queued: row.queued,
            oldest_due_age: row
                .oldest_due
                .map(|oldest| (now - oldest).max(Duration::ZERO)),
        })
    }
}
