//! Email addresses: the sign-in identifier (BR-1.2).

/// A normalised (trimmed, lower-cased) email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Email(String);

/// Why an email address was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EmailError {
    /// Nothing was entered.
    #[error("enter an email address")]
    Empty,
    /// Longer than 254 characters.
    #[error("the email address is too long")]
    TooLong,
    /// Not a plausible address.
    #[error("enter a valid email address")]
    Invalid,
}

const LOCAL_SPECIALS: &str = ".!#$%&'*+/=?^_`{|}~-";

impl Email {
    /// Validate and normalise an address.
    ///
    /// # Errors
    /// [`EmailError`] describing the first problem found.
    pub fn parse(raw: &str) -> Result<Self, EmailError> {
        let value = raw.trim().to_lowercase();
        if value.is_empty() {
            return Err(EmailError::Empty);
        }
        if value.chars().count() > 254 {
            return Err(EmailError::TooLong);
        }
        let (local, domain) = value.split_once('@').ok_or(EmailError::Invalid)?;
        if domain.contains('@') || local.is_empty() || local.chars().count() > 64 {
            return Err(EmailError::Invalid);
        }
        let local_ok = local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || LOCAL_SPECIALS.contains(c))
            && !local.starts_with('.')
            && !local.ends_with('.')
            && !local.contains("..");
        let domain_ok = domain.contains('.')
            && domain.split('.').all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            });
        if local_ok && domain_ok {
            Ok(Self(value))
        } else {
            Err(EmailError::Invalid)
        }
    }

    /// The normalised address.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_1_2_email_is_trimmed_and_lower_cased() {
        assert_eq!(
            Email::parse("  Ana.Lee@Example.COM ").unwrap().as_str(),
            "ana.lee@example.com"
        );
    }

    #[test]
    fn br_1_2_rejects_malformed_addresses() {
        for bad in [
            "",
            "   ",
            "no-at-sign",
            "a@b",
            "@example.com",
            "a@@example.com",
            "a b@example.com",
            "a@exa mple.com",
            ".a@example.com",
            "a..b@example.com",
            "a@-example.com",
            "a@example..com",
        ] {
            assert!(Email::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn br_1_2_enforces_length_limits() {
        let long_local = format!("{}@example.com", "a".repeat(65));
        assert_eq!(Email::parse(&long_local), Err(EmailError::Invalid));
        let too_long = format!("a@{}.com", "b".repeat(260));
        assert_eq!(Email::parse(&too_long), Err(EmailError::TooLong));
    }

    #[test]
    fn br_1_2_case_variants_are_the_same_identifier() {
        assert_eq!(
            Email::parse("A@Example.com").unwrap(),
            Email::parse("a@example.COM").unwrap()
        );
    }
}
