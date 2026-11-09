//! Skeleton chat storage.

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use tandem_core::chat::{Author, ChatMessage, ChatSession};
use tandem_core::ids::{ChatMessageId, ChatSessionId, UserId};
use tandem_ports::{ChatRepo, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

struct MessageRow {
    id: Uuid,
    chat_session_id: Uuid,
    author_kind: String,
    author_user_id: Option<Uuid>,
    author_agent: Option<String>,
    body: String,
    created_at: OffsetDateTime,
    in_reply_to: Option<Uuid>,
}

impl MessageRow {
    fn into_message(self) -> Result<ChatMessage, StoreError> {
        let author = match (
            self.author_kind.as_str(),
            self.author_user_id,
            self.author_agent,
        ) {
            ("user", Some(user), None) => Author::User(UserId::from_uuid(user)),
            ("agent", None, Some(agent)) => Author::Agent(agent),
            _ => return Err(corrupt("message author")),
        };
        Ok(ChatMessage {
            id: ChatMessageId::from_uuid(self.id),
            chat_session_id: ChatSessionId::from_uuid(self.chat_session_id),
            author,
            body: self.body,
            created_at: self.created_at,
            in_reply_to: self.in_reply_to.map(ChatMessageId::from_uuid),
        })
    }
}

fn session(row: (Uuid, String, Uuid, OffsetDateTime)) -> ChatSession {
    ChatSession {
        id: ChatSessionId::from_uuid(row.0),
        title: row.1,
        owner_id: UserId::from_uuid(row.2),
        created_at: row.3,
    }
}

#[async_trait]
impl ChatRepo for PgTx {
    async fn insert_session(&mut self, chat_session: &ChatSession) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO chat_sessions (id, title, owner_id, created_at) VALUES ($1, $2, $3, $4)",
            chat_session.id.as_uuid(),
            chat_session.title,
            chat_session.owner_id.as_uuid(),
            chat_session.created_at,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get_session(&mut self, id: ChatSessionId) -> Result<Option<ChatSession>, StoreError> {
        let row = sqlx::query!(
            "SELECT id, title, owner_id, created_at FROM chat_sessions WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(|row| session((row.id, row.title, row.owner_id, row.created_at))))
    }

    async fn list_sessions_for_owner(
        &mut self,
        owner: UserId,
    ) -> Result<Vec<ChatSession>, StoreError> {
        let rows = sqlx::query!(
            "SELECT id, title, owner_id, created_at FROM chat_sessions WHERE owner_id = $1
             ORDER BY created_at DESC, id DESC",
            owner.as_uuid()
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(rows
            .into_iter()
            .map(|row| session((row.id, row.title, row.owner_id, row.created_at)))
            .collect())
    }

    async fn insert_message(&mut self, message: &ChatMessage) -> Result<(), StoreError> {
        let (kind, user, agent): (&str, Option<Uuid>, Option<&str>) = match &message.author {
            Author::User(id) => ("user", Some(id.as_uuid()), None),
            Author::Agent(name) => ("agent", None, Some(name.as_str())),
        };
        sqlx::query!(
            "INSERT INTO chat_messages (id, chat_session_id, author_kind, author_user_id, author_agent, body,
                                        created_at, in_reply_to)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            message.id.as_uuid(),
            message.chat_session_id.as_uuid(),
            kind,
            user,
            agent,
            message.body,
            message.created_at,
            message.in_reply_to.map(|id| id.as_uuid()),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get_message(&mut self, id: ChatMessageId) -> Result<Option<ChatMessage>, StoreError> {
        sqlx::query_as!(
            MessageRow,
            "SELECT id, chat_session_id, author_kind, author_user_id, author_agent, body, created_at, in_reply_to
             FROM chat_messages WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(MessageRow::into_message)
        .transpose()
    }

    async fn list_messages(
        &mut self,
        session_id: ChatSessionId,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, StoreError> {
        sqlx::query_as!(
            MessageRow,
            "SELECT id, chat_session_id, author_kind, author_user_id, author_agent, body, created_at, in_reply_to
             FROM chat_messages WHERE chat_session_id = $1 ORDER BY seq LIMIT $2",
            session_id.as_uuid(),
            i64::try_from(limit).unwrap_or(i64::MAX),
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .into_iter()
        .map(MessageRow::into_message)
        .collect()
    }

    async fn has_agent_reply_to(&mut self, message_id: ChatMessageId) -> Result<bool, StoreError> {
        let row = sqlx::query!(
            r#"SELECT EXISTS (SELECT 1 FROM chat_messages WHERE in_reply_to = $1) AS "exists!""#,
            message_id.as_uuid()
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.exists)
    }
}
