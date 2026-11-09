//! Display names (BR-1.3).

/// A user's display name: 1 to 80 characters, no control characters, no surrounding spaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayName(String);

/// Why a display name was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DisplayNameError {
    /// Nothing was entered.
    #[error("enter a name")]
    Empty,
    /// Longer than 80 characters.
    #[error("enter a name up to 80 characters")]
    TooLong,
    /// Contains control characters.
    #[error("the name contains characters that are not allowed")]
    InvalidCharacters,
}

impl DisplayName {
    /// Maximum length in characters.
    pub const MAX_CHARS: usize = 80;

    /// Validate a name (surrounding spaces are removed).
    ///
    /// # Errors
    /// [`DisplayNameError`] describing the problem.
    pub fn parse(raw: &str) -> Result<Self, DisplayNameError> {
        let value = raw.trim();
        if value.is_empty() {
            return Err(DisplayNameError::Empty);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DisplayNameError::TooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(DisplayNameError::InvalidCharacters);
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_1_3_trims_and_accepts_one_to_eighty_characters() {
        assert_eq!(
            DisplayName::parse("  Ana Lee ").unwrap().as_str(),
            "Ana Lee"
        );
        assert!(DisplayName::parse(&"é".repeat(80)).is_ok());
    }

    #[test]
    fn br_1_3_rejects_empty_long_and_control_characters() {
        assert_eq!(DisplayName::parse("   "), Err(DisplayNameError::Empty));
        assert_eq!(
            DisplayName::parse(&"a".repeat(81)),
            Err(DisplayNameError::TooLong)
        );
        assert_eq!(
            DisplayName::parse("a\nb"),
            Err(DisplayNameError::InvalidCharacters)
        );
    }
}
