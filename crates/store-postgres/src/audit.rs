//! The audit log: append and read only. The application role has no UPDATE or DELETE privilege on
//! the table, and this module has no such query (SECURITY-14).

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use std::collections::BTreeMap;
use tandem_core::audit::{AuditEntry, AuditEvent, AuditEventType, AuditOutcome};
use tandem_core::ids::{AuditEntryId, UserId};
use tandem_ports::{AuditCursor, AuditFilter, AuditPage, AuditRepo, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

struct AuditRow {
    id: Uuid,
    occurred_at: OffsetDateTime,
    actor_id: Option<Uuid>,
    actor_label: String,
    event_type: String,
    object_type: Option<String>,
    object_id: Option<String>,
    outcome: String,
    details: serde_json::Value,
    request_id: Option<String>,
    source: Option<String>,
}

impl AuditRow {
    fn into_entry(self) -> Result<AuditEntry, StoreError> {
        let details: BTreeMap<String, String> =
            serde_json::from_value(self.details).map_err(|_| corrupt("audit details"))?;
        Ok(AuditEntry {
            id: AuditEntryId::from_uuid(self.id),
            event: AuditEvent {
                occurred_at: self.occurred_at,
                actor_id: self.actor_id.map(UserId::from_uuid),
                actor_label: self.actor_label,
                event_type: AuditEventType::parse(&self.event_type)
                    .ok_or_else(|| corrupt("audit event type"))?,
                object_type: self.object_type,
                object_id: self.object_id,
                outcome: AuditOutcome::parse(&self.outcome)
                    .ok_or_else(|| corrupt("audit outcome"))?,
                details,
                request_id: self.request_id,
                source: self.source,
            },
        })
    }
}

#[async_trait]
impl AuditRepo for PgTx {
    async fn append(&mut self, entry: &AuditEntry) -> Result<(), StoreError> {
        let event = &entry.event;
        let details = serde_json::to_value(&event.details)
            .map_err(|error| StoreError::Other(Box::new(error)))?;
        sqlx::query!(
            "INSERT INTO audit_entries (id, occurred_at, actor_id, actor_label, event_type, object_type, object_id,
                                        outcome, details, request_id, source)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            entry.id.as_uuid(),
            event.occurred_at,
            event.actor_id.map(|id| id.as_uuid()),
            event.actor_label,
            event.event_type.as_str(),
            event.object_type,
            event.object_id,
            event.outcome.as_str(),
            details,
            event.request_id,
            event.source,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn query(
        &mut self,
        filter: &AuditFilter,
        after: Option<AuditCursor>,
        limit: usize,
    ) -> Result<AuditPage, StoreError> {
        let fetch = i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX);
        let rows = sqlx::query_as!(
            AuditRow,
            "SELECT id, occurred_at, actor_id, actor_label, event_type, object_type, object_id, outcome, details,
                    request_id, source
             FROM audit_entries
             WHERE ($1::uuid IS NULL OR actor_id = $1)
               AND ($2::text IS NULL OR event_type = $2)
               AND ($3::timestamptz IS NULL OR occurred_at >= $3)
               AND ($4::timestamptz IS NULL OR occurred_at <= $4)
               AND ($5::timestamptz IS NULL OR (occurred_at, id) < ($5::timestamptz, $6::uuid))
             ORDER BY occurred_at DESC, id DESC
             LIMIT $7",
            filter.actor_id.map(|id| id.as_uuid()),
            filter.event_type.map(AuditEventType::as_str),
            filter.from,
            filter.to,
            after.map(|cursor| cursor.occurred_at),
            after.map(|cursor| cursor.id.as_uuid()),
            fetch,
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        let has_more = rows.len() > limit;
        let entries: Vec<AuditEntry> = rows
            .into_iter()
            .take(limit)
            .map(AuditRow::into_entry)
            .collect::<Result<_, _>>()?;
        let next = if has_more {
            entries.last().map(|entry| AuditCursor {
                occurred_at: entry.event.occurred_at,
                id: entry.id,
            })
        } else {
            None
        };
        Ok(AuditPage { entries, next })
    }
}
