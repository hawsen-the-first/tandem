//! Validity periods (BR-5.2, BR-6.1, BR-8.2).

use time::{Duration, OffsetDateTime};

/// An invitation link is valid for 7 days (assumption A7).
pub const INVITATION_VALIDITY: Duration = Duration::days(7);
/// A password reset link is valid for 24 hours (assumption A7).
pub const RESET_VALIDITY: Duration = Duration::hours(24);

/// When an invitation created at `created_at` expires.
pub fn invitation_expires_at(created_at: OffsetDateTime) -> OffsetDateTime {
    created_at + INVITATION_VALIDITY
}

/// When a reset link created at `created_at` expires.
pub fn reset_expires_at(created_at: OffsetDateTime) -> OffsetDateTime {
    created_at + RESET_VALIDITY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_5_2_invitations_last_seven_days_and_br_6_1_resets_last_a_day() {
        let start = OffsetDateTime::UNIX_EPOCH;
        assert_eq!(invitation_expires_at(start) - start, Duration::days(7));
        assert_eq!(reset_expires_at(start) - start, Duration::hours(24));
    }
}
