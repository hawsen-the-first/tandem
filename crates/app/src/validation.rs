//! Shared input validation: names, emails and passwords.

use crate::deps::App;
use crate::error::{AppError, FieldIssue};
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::password_policy::{self, PasswordViolation};
use tandem_core::settings::SecuritySettings;
use tandem_ports::Tx;

/// The message shown for one password violation (never echoes the password, BR-2.6).
pub(crate) fn violation_message(violation: PasswordViolation) -> String {
    match violation {
        PasswordViolation::TooShort => "Use at least 10 characters".to_owned(),
        PasswordViolation::TooLong => "Use at most 128 characters".to_owned(),
        PasswordViolation::MissingUpperCase => "Add an upper-case letter".to_owned(),
        PasswordViolation::MissingLowerCase => "Add a lower-case letter".to_owned(),
        PasswordViolation::MissingDigit => "Add a digit".to_owned(),
        PasswordViolation::MissingSymbol => "Add a symbol".to_owned(),
        PasswordViolation::Breached => {
            "This password has appeared in a data breach. Choose another".to_owned()
        }
        PasswordViolation::SameAsCurrent => {
            "Choose a password different from your current one".to_owned()
        }
    }
}

/// Parse a display name and an email, collecting every problem.
pub(crate) fn name_and_email(
    display_name: &str,
    email: &str,
    issues: &mut Vec<FieldIssue>,
) -> (Option<DisplayName>, Option<Email>) {
    let name = match DisplayName::parse(display_name) {
        Ok(name) => Some(name),
        Err(error) => {
            issues.push(FieldIssue {
                field: "display_name",
                message: error.to_string(),
            });
            None
        }
    };
    let email = match Email::parse(email) {
        Ok(email) => Some(email),
        Err(error) => {
            issues.push(FieldIssue {
                field: "email",
                message: error.to_string(),
            });
            None
        }
    };
    (name, email)
}

impl App {
    /// Load the instance security settings (defaults when nothing is stored, BR-10.2).
    pub(crate) async fn load_settings(tx: &mut dyn Tx) -> Result<SecuritySettings, AppError> {
        let stored = tx.settings().get_all().await?;
        Ok(SecuritySettings::from_stored(&stored))
    }

    /// Check a new password against the character rules, the breach lists and (optionally) the
    /// current password. Returns the issues found, empty when acceptable.
    pub(crate) async fn password_issues(
        &self,
        password: &str,
        online_breach_check: bool,
        current_hash: Option<&str>,
    ) -> Result<Vec<FieldIssue>, AppError> {
        let mut violations = password_policy::check(password);
        if violations.is_empty() {
            if self.breach.is_breached(password, online_breach_check).await {
                violations.push(PasswordViolation::Breached);
            }
            if let Some(hash) = current_hash
                && self.hasher.verify(password, hash).await?.matches
            {
                violations.push(PasswordViolation::SameAsCurrent);
            }
        }
        Ok(violations
            .into_iter()
            .map(|violation| FieldIssue {
                field: "password",
                message: violation_message(violation),
            })
            .collect())
    }
}

/// Turn collected issues into an error, or `Ok` when there are none.
pub(crate) fn fail_if_any(issues: Vec<FieldIssue>) -> Result<(), AppError> {
    if issues.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(issues))
    }
}
