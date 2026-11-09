//! Password policy (BR-2.1, BR-2.2, BR-2.4). The breached-password check (BR-2.3) needs data and
//! lives behind the `BreachChecker` port; its result is reported as [`PasswordViolation::Breached`].

/// Shortest allowed password, in characters.
pub const MIN_LENGTH: usize = 10;
/// Longest allowed password, in characters.
pub const MAX_LENGTH: usize = 128;

/// A way in which a password fails the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasswordViolation {
    /// Fewer than [`MIN_LENGTH`] characters.
    TooShort,
    /// More than [`MAX_LENGTH`] characters.
    TooLong,
    /// No upper-case letter.
    MissingUpperCase,
    /// No lower-case letter.
    MissingLowerCase,
    /// No digit.
    MissingDigit,
    /// No symbol (a character that is neither a letter nor a digit).
    MissingSymbol,
    /// Found in the breached-password list.
    Breached,
    /// The same as the current password.
    SameAsCurrent,
}

impl PasswordViolation {
    /// The unmet requirement in plain words (never echoes the password, BR-2.6).
    pub const fn requirement(self) -> &'static str {
        match self {
            Self::TooShort => "at least 10 characters",
            Self::TooLong => "at most 128 characters",
            Self::MissingUpperCase => "an upper-case letter",
            Self::MissingLowerCase => "a lower-case letter",
            Self::MissingDigit => "a digit",
            Self::MissingSymbol => "a symbol",
            Self::Breached => "a password that has not appeared in known data breaches",
            Self::SameAsCurrent => "a password different from the current one",
        }
    }
}

/// Check the character rules and return every violation (empty means the password passes).
pub fn check(password: &str) -> Vec<PasswordViolation> {
    let mut violations = Vec::new();
    let length = password.chars().count();
    if length < MIN_LENGTH {
        violations.push(PasswordViolation::TooShort);
    }
    if length > MAX_LENGTH {
        violations.push(PasswordViolation::TooLong);
    }
    if !password.chars().any(char::is_uppercase) {
        violations.push(PasswordViolation::MissingUpperCase);
    }
    if !password.chars().any(char::is_lowercase) {
        violations.push(PasswordViolation::MissingLowerCase);
    }
    if !password.chars().any(char::is_numeric) {
        violations.push(PasswordViolation::MissingDigit);
    }
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        violations.push(PasswordViolation::MissingSymbol);
    }
    violations
}

#[cfg(test)]
mod tests;
