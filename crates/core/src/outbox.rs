//! Outbox events: written in the same transaction as the change they describe (BR-13.4).

use crate::ids::{ChatMessageId, ChatSessionId, OutboxEventId, UserId};
use time::OffsetDateTime;

/// What happened, with the data consumers need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// A skeleton message was posted.
    MessagePosted {
        /// The session.
        chat_session_id: ChatSessionId,
        /// The message.
        message_id: ChatMessageId,
        /// The session owner, who is allowed to see the event.
        owner_id: UserId,
        /// Whether an agent wrote it (agent messages never trigger another reply).
        author_is_agent: bool,
    },
}

impl EventPayload {
    /// Stable event kind used in storage.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::MessagePosted { .. } => "message.posted",
        }
    }

    /// The user allowed to see this event over the event stream.
    pub const fn visible_to(&self) -> UserId {
        match self {
            Self::MessagePosted { owner_id, .. } => *owner_id,
        }
    }
}

/// An event to be written to the outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutboxEvent {
    /// The payload.
    pub payload: EventPayload,
    /// Trace context so a trace follows the event from `api` to `worker`.
    pub trace_context: Option<String>,
}

/// A stored outbox event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEvent {
    /// Sequence number assigned by the store.
    pub id: OutboxEventId,
    /// When written.
    pub created_at: OffsetDateTime,
    /// The payload.
    pub payload: EventPayload,
    /// Trace context.
    pub trace_context: Option<String>,
    /// Set once the relay has published it.
    pub published_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    #[test]
    fn events_are_visible_only_to_the_session_owner() {
        let clock = FakeClock::at_unix(0);
        let rng = FakeRng::new(3);
        let owner = UserId::generate(&clock, &rng).unwrap();
        let payload = EventPayload::MessagePosted {
            chat_session_id: ChatSessionId::generate(&clock, &rng).unwrap(),
            message_id: ChatMessageId::generate(&clock, &rng).unwrap(),
            owner_id: owner,
            author_is_agent: false,
        };
        assert_eq!(payload.kind(), "message.posted");
        assert_eq!(payload.visible_to(), owner);
    }
}
