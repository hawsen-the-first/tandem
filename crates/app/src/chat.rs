//! Walking-skeleton chat services (W15). Replaced by the full collaboration model in U3.

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::chat::{Author, ChatMessage, ChatSession, validate_body, validate_title};
use tandem_core::ids::{ChatMessageId, ChatSessionId};
use tandem_core::outbox::{EventPayload, NewOutboxEvent};
use tandem_core::permissions::Action;

impl App {
    /// Create a skeleton session owned by the caller.
    ///
    /// # Errors
    /// [`AppError::Validation`] for a bad title.
    pub async fn create_chat_session(
        &self,
        ctx: &RequestContext,
        title: &str,
    ) -> Result<ChatSession, AppError> {
        self.authorize(ctx, Action::UseChat).await?;
        let actor = ctx.actor()?;
        let title = validate_title(title)
            .map_err(|error| AppError::validation("title", error.to_string()))?;
        let session = ChatSession {
            id: ChatSessionId::generate(&*self.clock, &*self.rng)?,
            title,
            owner_id: actor.user_id,
            created_at: self.clock.now(),
        };
        let mut tx = self.store.begin().await?;
        tx.chat().insert_session(&session).await?;
        tx.commit().await?;
        Ok(session)
    }

    /// The caller's own sessions, newest first (BR-13.1).
    ///
    /// # Errors
    /// [`AppError::Forbidden`] for an enrolment-only sign-in.
    pub async fn list_chat_sessions(
        &self,
        ctx: &RequestContext,
    ) -> Result<Vec<ChatSession>, AppError> {
        self.authorize(ctx, Action::UseChat).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        Ok(tx.chat().list_sessions_for_owner(actor.user_id).await?)
    }

    /// Post a message in the caller's own session. The message and an outbox event are written in
    /// one transaction (BR-13.4).
    ///
    /// # Errors
    /// [`AppError::NotFound`] when the session is not the caller's; [`AppError::Validation`] for a
    /// bad body.
    pub async fn post_message(
        &self,
        ctx: &RequestContext,
        session_id: ChatSessionId,
        body: &str,
    ) -> Result<ChatMessage, AppError> {
        self.authorize(ctx, Action::UseChat).await?;
        let actor = ctx.actor()?;
        let body =
            validate_body(body).map_err(|error| AppError::validation("body", error.to_string()))?;
        let mut tx = self.store.begin().await?;
        let session = tx
            .chat()
            .get_session(session_id)
            .await?
            .filter(|session| session.owner_id == actor.user_id)
            .ok_or(AppError::NotFound)?;
        let message = ChatMessage {
            id: ChatMessageId::generate(&*self.clock, &*self.rng)?,
            chat_session_id: session.id,
            author: Author::User(actor.user_id),
            body,
            created_at: self.clock.now(),
            in_reply_to: None,
        };
        tx.chat().insert_message(&message).await?;
        let event = NewOutboxEvent {
            payload: EventPayload::MessagePosted {
                chat_session_id: session.id,
                message_id: message.id,
                owner_id: session.owner_id,
                author_is_agent: false,
            },
            trace_context: ctx.trace_context.clone(),
        };
        tx.outbox().append(&event, message.created_at).await?;
        tx.commit().await?;
        Ok(message)
    }

    /// The messages of the caller's own session, oldest first.
    ///
    /// # Errors
    /// [`AppError::NotFound`] when the session is not the caller's.
    pub async fn list_messages(
        &self,
        ctx: &RequestContext,
        session_id: ChatSessionId,
    ) -> Result<Vec<ChatMessage>, AppError> {
        self.authorize(ctx, Action::UseChat).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        tx.chat()
            .get_session(session_id)
            .await?
            .filter(|session| session.owner_id == actor.user_id)
            .ok_or(AppError::NotFound)?;
        Ok(tx.chat().list_messages(session_id, 500).await?)
    }
}
