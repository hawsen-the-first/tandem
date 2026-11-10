//! Checks that run before a destructive command touches anything.

use crate::error::CliError;
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime, Time};

/// Audit entries younger than this are never pruned (SE-15: retention of at least 90 days).
pub const MIN_AUDIT_RETENTION: Duration = Duration::days(90);

/// Refuse a destructive command that was not confirmed with `--yes`.
///
/// # Errors
/// [`CliError::NotConfirmed`] naming what would have happened.
pub fn require_confirmation(confirmed: bool, action: &str) -> Result<(), CliError> {
    if confirmed {
        Ok(())
    } else {
        Err(CliError::NotConfirmed(format!(
            "{action}: pass --yes to confirm"
        )))
    }
}

/// Parse `--before`: an RFC 3339 timestamp or a plain `YYYY-MM-DD` date (midnight UTC).
///
/// # Errors
/// A message for clap when neither form matches.
pub fn parse_cutoff(text: &str) -> Result<OffsetDateTime, String> {
    if let Ok(timestamp) = OffsetDateTime::parse(text, &Rfc3339) {
        return Ok(timestamp);
    }
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Date::parse(text, &format)
        .map(|date| date.with_time(Time::MIDNIGHT).assume_utc())
        .map_err(|_| "expected an RFC 3339 timestamp or a YYYY-MM-DD date".to_owned())
}

/// Refuse a cutoff inside the minimum retention window.
///
/// # Errors
/// [`CliError::BadArgument`] when `cutoff` is newer than `now` minus 90 days.
pub fn check_prune_cutoff(cutoff: OffsetDateTime, now: OffsetDateTime) -> Result<(), CliError> {
    if cutoff > now - MIN_AUDIT_RETENTION {
        Err(CliError::BadArgument(
            "--before must be at least 90 days in the past; audit entries are kept for 90 days or more"
                .to_owned(),
        ))
    } else {
        Ok(())
    }
}
