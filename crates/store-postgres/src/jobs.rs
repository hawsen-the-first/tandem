//! The job queue: `FOR UPDATE SKIP LOCKED` leasing, heartbeat, retry and lapsed-lease recovery
//! (SC-3, RL-5). Domain transitions stay in `tandem_core::job`; this module stores their results.

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use serde_json::{Value, json};
use tandem_core::ids::{ChatMessageId, JobId};
use tandem_core::job::{Job, JobKind, JobStatus};
use tandem_ports::{JobQueue, StoreError};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn status_text(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Leased => "leased",
        JobStatus::Done => "done",
        JobStatus::Failed => "failed",
    }
}

fn parse_status(text: &str) -> Result<JobStatus, StoreError> {
    match text {
        "queued" => Ok(JobStatus::Queued),
        "leased" => Ok(JobStatus::Leased),
        "done" => Ok(JobStatus::Done),
        "failed" => Ok(JobStatus::Failed),
        _ => Err(corrupt("job status")),
    }
}

fn kind_to_json(kind: &JobKind) -> Value {
    match kind {
        JobKind::MockAgentReply { message_id } => json!({ "message_id": message_id.to_string() }),
    }
}

fn kind_from_json(name: &str, payload: &Value) -> Result<JobKind, StoreError> {
    match name {
        "mock_agent_reply" => {
            let id = payload
                .get("message_id")
                .and_then(Value::as_str)
                .ok_or_else(|| corrupt("job payload"))?;
            Ok(JobKind::MockAgentReply {
                message_id: id
                    .parse::<ChatMessageId>()
                    .map_err(|_| corrupt("job payload"))?,
            })
        }
        _ => Err(corrupt("job kind")),
    }
}

struct JobRow {
    id: Uuid,
    kind: String,
    payload: Value,
    status: String,
    attempts: i32,
    run_at: OffsetDateTime,
    last_error: Option<String>,
    lease_expires_at: Option<OffsetDateTime>,
    trace_context: Option<String>,
}

impl JobRow {
    fn into_job(self) -> Result<Job, StoreError> {
        Ok(Job {
            id: JobId::from_uuid(self.id),
            kind: kind_from_json(&self.kind, &self.payload)?,
            status: parse_status(&self.status)?,
            attempts: u32::try_from(self.attempts).map_err(|_| corrupt("job attempts"))?,
            run_at: self.run_at,
            last_error: self.last_error,
            lease_expires_at: self.lease_expires_at,
            trace_context: self.trace_context,
        })
    }
}

#[async_trait]
impl JobQueue for PgTx {
    async fn enqueue(&mut self, job: &Job) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO jobs (id, kind, payload, status, attempts, run_at, last_error, lease_expires_at,
                               trace_context)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            job.id.as_uuid(),
            job.kind.name(),
            kind_to_json(&job.kind),
            status_text(job.status),
            i32::try_from(job.attempts).map_err(|_| corrupt("job attempts"))?,
            job.run_at,
            job.last_error,
            job.lease_expires_at,
            job.trace_context,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn lease_next(
        &mut self,
        now: OffsetDateTime,
        ttl: Duration,
    ) -> Result<Option<Job>, StoreError> {
        sqlx::query_as!(
            JobRow,
            "UPDATE jobs SET status = 'leased', attempts = attempts + 1, lease_expires_at = $2
             WHERE id = (
                 SELECT id FROM jobs WHERE status = 'queued' AND run_at <= $1
                 ORDER BY run_at, id FOR UPDATE SKIP LOCKED LIMIT 1)
             RETURNING id, kind, payload, status, attempts, run_at, last_error, lease_expires_at, trace_context",
            now,
            now + ttl,
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(JobRow::into_job)
        .transpose()
    }

    async fn save(&mut self, job: &Job) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE jobs SET status = $2, attempts = $3, run_at = $4, last_error = $5, lease_expires_at = $6,
                    trace_context = $7
             WHERE id = $1",
            job.id.as_uuid(),
            status_text(job.status),
            i32::try_from(job.attempts).map_err(|_| corrupt("job attempts"))?,
            job.run_at,
            job.last_error,
            job.lease_expires_at,
            job.trace_context,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn get(&mut self, id: JobId) -> Result<Option<Job>, StoreError> {
        sqlx::query_as!(
            JobRow,
            "SELECT id, kind, payload, status, attempts, run_at, last_error, lease_expires_at, trace_context
             FROM jobs WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(JobRow::into_job)
        .transpose()
    }

    async fn recover_lapsed_leases(&mut self, now: OffsetDateTime) -> Result<u64, StoreError> {
        let rows = sqlx::query_as!(
            JobRow,
            "SELECT id, kind, payload, status, attempts, run_at, last_error, lease_expires_at, trace_context
             FROM jobs WHERE status = 'leased' AND lease_expires_at <= $1 FOR UPDATE SKIP LOCKED",
            now
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        let mut recovered = 0;
        for row in rows {
            let mut job = row.into_job()?;
            if job.recover_if_lease_lapsed(now).is_some() {
                self.save(&job).await?;
                recovered += 1;
            }
        }
        Ok(recovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    #[test]
    fn job_kinds_round_trip_through_json() {
        let kind = JobKind::MockAgentReply {
            message_id: ChatMessageId::generate(&FakeClock::at_unix(0), &FakeRng::new(1)).unwrap(),
        };
        assert_eq!(
            kind_from_json(kind.name(), &kind_to_json(&kind)).unwrap(),
            kind
        );
        assert!(kind_from_json("other", &json!({})).is_err());
    }

    #[test]
    fn statuses_round_trip() {
        for status in [
            JobStatus::Queued,
            JobStatus::Leased,
            JobStatus::Done,
            JobStatus::Failed,
        ] {
            assert_eq!(parse_status(status_text(status)).unwrap(), status);
        }
        assert!(parse_status("nope").is_err());
    }
}
