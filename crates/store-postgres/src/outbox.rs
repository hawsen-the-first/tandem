//! The transactional outbox (BR-13.4). Appending also queues a `NOTIFY` that PostgreSQL delivers
//! when the transaction commits; listeners fetch rows by id, so a lost notification only adds
//! latency (NFR design 3).

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use serde_json::{Value, json};
use tandem_core::ids::{ChatMessageId, ChatSessionId, OutboxEventId, UserId};
use tandem_core::outbox::{EventPayload, NewOutboxEvent, OutboxEvent};
use tandem_ports::{OutboxRepo, StoreError};
use time::OffsetDateTime;

/// The channel listeners subscribe to.
pub const NOTIFY_CHANNEL: &str = "tandem_events";

fn payload_to_json(payload: &EventPayload) -> Value {
    match payload {
        EventPayload::MessagePosted {
            chat_session_id,
            message_id,
            owner_id,
            author_is_agent,
        } => json!({
            "chat_session_id": chat_session_id.to_string(),
            "message_id": message_id.to_string(),
            "owner_id": owner_id.to_string(),
            "author_is_agent": author_is_agent,
        }),
    }
}

fn payload_from_json(kind: &str, value: &Value) -> Result<EventPayload, StoreError> {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| corrupt("outbox payload"))
    };
    match kind {
        "message.posted" => Ok(EventPayload::MessagePosted {
            chat_session_id: text("chat_session_id")?
                .parse::<ChatSessionId>()
                .map_err(|_| corrupt("outbox payload"))?,
            message_id: text("message_id")?
                .parse::<ChatMessageId>()
                .map_err(|_| corrupt("outbox payload"))?,
            owner_id: text("owner_id")?
                .parse::<UserId>()
                .map_err(|_| corrupt("outbox payload"))?,
            author_is_agent: value
                .get("author_is_agent")
                .and_then(Value::as_bool)
                .ok_or_else(|| corrupt("outbox payload"))?,
        }),
        _ => Err(corrupt("outbox event kind")),
    }
}

struct OutboxRow {
    id: i64,
    kind: String,
    payload: Value,
    trace_context: Option<String>,
    created_at: OffsetDateTime,
    published_at: Option<OffsetDateTime>,
}

impl OutboxRow {
    fn into_event(self) -> Result<OutboxEvent, StoreError> {
        Ok(OutboxEvent {
            id: OutboxEventId(self.id),
            created_at: self.created_at,
            payload: payload_from_json(&self.kind, &self.payload)?,
            trace_context: self.trace_context,
            published_at: self.published_at,
        })
    }
}

#[async_trait]
impl OutboxRepo for PgTx {
    async fn append(
        &mut self,
        event: &NewOutboxEvent,
        now: OffsetDateTime,
    ) -> Result<OutboxEventId, StoreError> {
        let row = sqlx::query!(
            "INSERT INTO outbox_events (kind, payload, trace_context, created_at) VALUES ($1, $2, $3, $4)
             RETURNING id",
            event.payload.kind(),
            payload_to_json(&event.payload),
            event.trace_context,
            now,
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(NOTIFY_CHANNEL)
            .bind(row.id.to_string())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(OutboxEventId(row.id))
    }

    async fn fetch_unpublished(&mut self, limit: usize) -> Result<Vec<OutboxEvent>, StoreError> {
        sqlx::query_as!(
            OutboxRow,
            "SELECT id, kind, payload, trace_context, created_at, published_at FROM outbox_events
             WHERE published_at IS NULL ORDER BY id LIMIT $1",
            i64::try_from(limit).unwrap_or(i64::MAX)
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .into_iter()
        .map(OutboxRow::into_event)
        .collect()
    }

    async fn mark_published(
        &mut self,
        ids: &[OutboxEventId],
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        let ids: Vec<i64> = ids.iter().map(|id| id.0).collect();
        sqlx::query!(
            "UPDATE outbox_events SET published_at = $2 WHERE id = ANY($1)",
            &ids,
            now
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn fetch_after(
        &mut self,
        after: OutboxEventId,
        limit: usize,
    ) -> Result<Vec<OutboxEvent>, StoreError> {
        sqlx::query_as!(
            OutboxRow,
            "SELECT id, kind, payload, trace_context, created_at, published_at FROM outbox_events
             WHERE id > $1 ORDER BY id LIMIT $2",
            after.0,
            i64::try_from(limit).unwrap_or(i64::MAX)
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .into_iter()
        .map(OutboxRow::into_event)
        .collect()
    }

    async fn lowest_retained_id(&mut self) -> Result<Option<OutboxEventId>, StoreError> {
        let row = sqlx::query!("SELECT min(id) AS lowest FROM outbox_events")
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(row.lowest.map(OutboxEventId))
    }

    async fn latest_id(&mut self) -> Result<Option<OutboxEventId>, StoreError> {
        let row = sqlx::query!("SELECT max(id) AS latest FROM outbox_events")
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(row.latest.map(OutboxEventId))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    #[test]
    fn payloads_round_trip_through_json() {
        let clock = FakeClock::at_unix(0);
        let rng = FakeRng::new(4);
        let payload = EventPayload::MessagePosted {
            chat_session_id: ChatSessionId::generate(&clock, &rng).unwrap(),
            message_id: ChatMessageId::generate(&clock, &rng).unwrap(),
            owner_id: UserId::generate(&clock, &rng).unwrap(),
            author_is_agent: true,
        };
        let json = payload_to_json(&payload);
        assert_eq!(payload_from_json(payload.kind(), &json).unwrap(), payload);
    }

    #[test]
    fn unknown_kinds_and_malformed_payloads_are_reported_as_corrupt() {
        assert!(payload_from_json("nope", &json!({})).is_err());
        assert!(payload_from_json("message.posted", &json!({"chat_session_id": 5})).is_err());
    }
}
