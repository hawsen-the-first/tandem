//! Configuration test runs and the per-target advisory lock (U2 NFR design pattern 2).

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use serde_json::{Map, Value, json};
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use tandem_core::config::{
    FailureReason, ProbeDetails, ProbeNote, ProbeResult, ProbeSubject, RunStatus, TestRun,
    TestStatus, TestTarget, ToolCalling,
};
use tandem_core::ids::{TestRunId, UserId};
use tandem_ports::{LockGuard, Locks, StoreError, TestRunRepo};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

fn note_text(note: ProbeNote) -> &'static str {
    match note {
        ProbeNote::NoResults => "no_results",
    }
}

fn result_to_json(result: &ProbeResult) -> Value {
    let details = &result.details;
    let mut map = Map::new();
    if let Some(tool_calling) = details.tool_calling {
        map.insert("tool_calling".into(), json!(tool_calling.as_str()));
    }
    if let Some(tokens) = details.context_tokens {
        map.insert("context_tokens".into(), json!(tokens));
    }
    if let Some(login) = &details.account_login {
        map.insert("account_login".into(), json!(login));
    }
    if let Some(expires) = details
        .token_expires_at
        .and_then(|at| at.format(&Rfc3339).ok())
    {
        map.insert("token_expires_at".into(), json!(expires));
    }
    if !details.repositories.is_empty() {
        map.insert("repositories".into(), json!(details.repositories));
    }
    if let Some(total) = details.repository_total {
        map.insert("repository_total".into(), json!(total));
    }
    if let Some(name) = &details.runtime_name {
        map.insert("runtime_name".into(), json!(name));
    }
    if let Some(version) = &details.runtime_version {
        map.insert("runtime_version".into(), json!(version));
    }
    json!({
        "subject": result.subject.key(),
        "status": result.status.as_str(),
        "note": result.note.map(note_text),
        "details": Value::Object(map),
    })
}

fn result_from_json(value: &Value) -> Result<ProbeResult, StoreError> {
    let text = |name: &str| value.get(name).and_then(Value::as_str);
    let subject = text("subject")
        .and_then(ProbeSubject::parse_key)
        .ok_or_else(|| corrupt("test subject"))?;
    let status = text("status")
        .and_then(TestStatus::parse)
        .ok_or_else(|| corrupt("test status"))?;
    let note = match text("note") {
        None => None,
        Some("no_results") => Some(ProbeNote::NoResults),
        Some(_) => return Err(corrupt("test note")),
    };
    let stored = value.get("details").cloned().unwrap_or(Value::Null);
    let detail_text = |name: &str| stored.get(name).and_then(Value::as_str).map(str::to_owned);
    let number = |name: &str| {
        stored
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|number| u32::try_from(number).ok())
    };
    let details = ProbeDetails {
        tool_calling: detail_text("tool_calling")
            .map(|text| ToolCalling::parse(&text).ok_or_else(|| corrupt("tool calling")))
            .transpose()?,
        context_tokens: number("context_tokens"),
        account_login: detail_text("account_login"),
        token_expires_at: detail_text("token_expires_at")
            .map(|text| OffsetDateTime::parse(&text, &Rfc3339).map_err(|_| corrupt("token expiry")))
            .transpose()?,
        repositories: stored
            .get("repositories")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        repository_total: number("repository_total"),
        runtime_name: detail_text("runtime_name"),
        runtime_version: detail_text("runtime_version"),
    };
    Ok(ProbeResult {
        subject,
        status,
        note,
        details,
    })
}

struct RunRow {
    id: Uuid,
    target: String,
    status: String,
    requested_by: Uuid,
    requested_at: OffsetDateTime,
    started_at: Option<OffsetDateTime>,
    finished_at: Option<OffsetDateTime>,
    results: Value,
    failure: Option<String>,
}

impl RunRow {
    fn into_run(self) -> Result<TestRun, StoreError> {
        let results = self
            .results
            .as_array()
            .ok_or_else(|| corrupt("test results"))?
            .iter()
            .map(result_from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TestRun {
            id: TestRunId::from_uuid(self.id),
            target: TestTarget::parse_key(&self.target).ok_or_else(|| corrupt("test target"))?,
            status: RunStatus::parse(&self.status).ok_or_else(|| corrupt("run status"))?,
            requested_by: UserId::from_uuid(self.requested_by),
            requested_at: self.requested_at,
            started_at: self.started_at,
            finished_at: self.finished_at,
            results,
            failure: self
                .failure
                .as_deref()
                .map(|text| FailureReason::parse(text).ok_or_else(|| corrupt("failure reason")))
                .transpose()?,
        })
    }
}

fn results_json(run: &TestRun) -> Value {
    Value::Array(run.results.iter().map(result_to_json).collect())
}

#[async_trait]
impl TestRunRepo for PgTx {
    async fn insert(&mut self, run: &TestRun) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO config_test_runs
                (id, target, status, requested_by, requested_at, started_at, finished_at, results, failure)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            run.id.as_uuid(),
            run.target.key(),
            run.status.as_str(),
            run.requested_by.as_uuid(),
            run.requested_at,
            run.started_at,
            run.finished_at,
            results_json(run),
            run.failure.map(FailureReason::as_str),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get(&mut self, id: TestRunId) -> Result<Option<TestRun>, StoreError> {
        let row = sqlx::query_as!(
            RunRow,
            "SELECT id, target, status, requested_by, requested_at, started_at, finished_at, results, failure
             FROM config_test_runs WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(RunRow::into_run).transpose()
    }

    async fn save(&mut self, run: &TestRun) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE config_test_runs
             SET status = $2, started_at = $3, finished_at = $4, results = $5, failure = $6
             WHERE id = $1",
            run.id.as_uuid(),
            run.status.as_str(),
            run.started_at,
            run.finished_at,
            results_json(run),
            run.failure.map(FailureReason::as_str),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn active_for(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError> {
        let row = sqlx::query_as!(
            RunRow,
            "SELECT id, target, status, requested_by, requested_at, started_at, finished_at, results, failure
             FROM config_test_runs WHERE target = $1 AND status IN ('queued', 'running')",
            target.key(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(RunRow::into_run).transpose()
    }

    async fn latest_finished(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError> {
        let row = sqlx::query_as!(
            RunRow,
            "SELECT id, target, status, requested_by, requested_at, started_at, finished_at, results, failure
             FROM config_test_runs
             WHERE target = $1 AND status IN ('succeeded', 'failed')
             ORDER BY requested_at DESC, id DESC LIMIT 1",
            target.key(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(RunRow::into_run).transpose()
    }

    async fn queued_before(&mut self, cutoff: OffsetDateTime) -> Result<Vec<TestRun>, StoreError> {
        let rows = sqlx::query_as!(
            RunRow,
            "SELECT id, target, status, requested_by, requested_at, started_at, finished_at, results, failure
             FROM config_test_runs WHERE status = 'queued' AND requested_at < $1
             ORDER BY requested_at",
            cutoff,
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(RunRow::into_run).collect()
    }

    async fn prune(
        &mut self,
        keep_per_target: u32,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StoreError> {
        let result = sqlx::query!(
            "WITH ranked AS (
                 SELECT id,
                        row_number() OVER (PARTITION BY target ORDER BY requested_at DESC, id DESC) AS position
                 FROM config_test_runs
                 WHERE status IN ('succeeded', 'failed'))
             DELETE FROM config_test_runs run USING ranked
             WHERE run.id = ranked.id AND ranked.position > 1
               AND (ranked.position > $1 OR run.requested_at < $2)",
            i64::from(keep_per_target),
            cutoff,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }
}

/// Session-level advisory locks on dedicated connections.
#[derive(Debug, Clone)]
pub struct PgLocks {
    pool: PgPool,
}

impl PgLocks {
    /// Locks taken on connections from `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// FNV-1a over the key, so every process and release derives the same lock number.
fn lock_number(key: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in b"tandem:config-test:".iter().chain(key.as_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    i64::from_ne_bytes(hash.to_ne_bytes())
}

struct PgLockGuard {
    connection: Option<PoolConnection<Postgres>>,
    number: i64,
}

#[async_trait]
impl LockGuard for PgLockGuard {
    async fn release(mut self: Box<Self>) {
        if let Some(mut connection) = self.connection.take() {
            let unlocked = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(self.number)
                .execute(&mut *connection)
                .await;
            if unlocked.is_err() {
                // A connection that cannot say it released the lock must not return to the pool.
                drop(connection.detach());
            }
        }
    }
}

impl Drop for PgLockGuard {
    fn drop(&mut self) {
        // Dropped without release (a crash path): closing the connection frees the lock.
        if let Some(connection) = self.connection.take() {
            drop(connection.detach());
        }
    }
}

#[async_trait]
impl Locks for PgLocks {
    async fn try_lock(&self, key: &str) -> Result<Option<Box<dyn LockGuard>>, StoreError> {
        let number = lock_number(key);
        let mut connection = self.pool.acquire().await.map_err(map_sqlx)?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(number)
            .fetch_one(&mut *connection)
            .await
            .map_err(map_sqlx)?;
        if !acquired {
            return Ok(None);
        }
        Ok(Some(Box::new(PgLockGuard {
            connection: Some(connection),
            number,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_numbers_are_stable_and_differ_by_key() {
        assert_eq!(lock_number("runtime"), lock_number("runtime"));
        assert_ne!(lock_number("runtime"), lock_number("search:brave"));
        // Pinned so a refactor cannot silently change the lock two workers must agree on.
        assert_eq!(lock_number("runtime"), -4_518_256_979_440_111_001);
    }

    #[test]
    fn results_survive_a_round_trip() {
        use tandem_core::ids::ProviderModelId;
        let mut result = ProbeResult::new(
            ProbeSubject::Model(
                "00000000-0000-0000-0000-000000000001"
                    .parse::<ProviderModelId>()
                    .unwrap_or_else(|_| unreachable!()),
            ),
            TestStatus::Ok,
        );
        result.note = Some(ProbeNote::NoResults);
        result.details = ProbeDetails {
            tool_calling: Some(ToolCalling::Supported),
            context_tokens: Some(32_768),
            account_login: Some("octocat".into()),
            token_expires_at: Some(time::macros::datetime!(2026-12-01 10:30:00 UTC)),
            repositories: vec!["a/b".into()],
            repository_total: Some(3),
            runtime_name: Some("Docker Engine".into()),
            runtime_version: Some("27.0.3".into()),
        };
        let back = result_from_json(&result_to_json(&result)).unwrap();
        assert_eq!(back, result);
    }
}
