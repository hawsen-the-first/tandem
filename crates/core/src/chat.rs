//! Walking-skeleton chat (replaced by the full collaboration model in U3). BR-13.1, BR-13.2.

use crate::ids::{ChatMessageId, ChatSessionId, UserId};
use time::OffsetDateTime;

/// Longest skeleton session title, in characters.
pub const TITLE_MAX_CHARS: usize = 120;
/// Longest skeleton message body, in characters.
pub const BODY_MAX_CHARS: usize = 4000;

/// Why chat input was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChatInputError {
    /// Empty title.
    #[error("enter a title")]
    EmptyTitle,
    /// Title over 120 characters.
    #[error("the title can be up to 120 characters")]
    TitleTooLong,
    /// Empty message.
    #[error("enter a message")]
    EmptyBody,
    /// Message over 4000 characters.
    #[error("a message can be up to 4000 characters")]
    BodyTooLong,
}

/// Validate and trim a title.
///
/// # Errors
/// [`ChatInputError`] when empty or too long.
pub fn validate_title(raw: &str) -> Result<String, ChatInputError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ChatInputError::EmptyTitle);
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(ChatInputError::TitleTooLong);
    }
    Ok(title.to_owned())
}

/// Validate a message body (plain text, kept as typed apart from outer whitespace).
///
/// # Errors
/// [`ChatInputError`] when empty or too long.
pub fn validate_body(raw: &str) -> Result<String, ChatInputError> {
    let body = raw.trim();
    if body.is_empty() {
        return Err(ChatInputError::EmptyBody);
    }
    if body.chars().count() > BODY_MAX_CHARS {
        return Err(ChatInputError::BodyTooLong);
    }
    Ok(body.to_owned())
}

/// A skeleton session: visible only to its owner (BR-13.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSession {
    /// Identifier.
    pub id: ChatSessionId,
    /// Title.
    pub title: String,
    /// The only user who can see it.
    pub owner_id: UserId,
    /// When created.
    pub created_at: OffsetDateTime,
}

/// Who wrote a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Author {
    /// A person.
    User(UserId),
    /// An agent, by name (`mock-agent` in U1).
    Agent(String),
}

/// A skeleton message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Identifier.
    pub id: ChatMessageId,
    /// The session it belongs to.
    pub chat_session_id: ChatSessionId,
    /// The author.
    pub author: Author,
    /// Plain-text body.
    pub body: String,
    /// When posted.
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_13_1_titles_are_one_to_one_hundred_twenty_characters() {
        assert_eq!(validate_title("  Plan  ").unwrap(), "Plan");
        assert_eq!(validate_title(" "), Err(ChatInputError::EmptyTitle));
        assert!(validate_title(&"a".repeat(120)).is_ok());
        assert_eq!(
            validate_title(&"a".repeat(121)),
            Err(ChatInputError::TitleTooLong)
        );
    }

    #[test]
    fn br_13_2_message_bodies_are_one_to_four_thousand_characters() {
        assert_eq!(validate_body(""), Err(ChatInputError::EmptyBody));
        assert!(validate_body(&"é".repeat(4000)).is_ok());
        assert_eq!(
            validate_body(&"é".repeat(4001)),
            Err(ChatInputError::BodyTooLong)
        );
    }
}
