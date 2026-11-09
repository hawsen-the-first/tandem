//! Password reset links issued by admins (BR-6).

use crate::expiry::reset_expires_at;
use crate::ids::{PasswordResetId, UserId};
use crate::token::TokenHash;
use time::OffsetDateTime;

/// Why a reset link could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResetError {
    /// Already used.
    #[error("this reset link was already used")]
    AlreadyUsed,
    /// Past its expiry.
    #[error("this reset link has expired")]
    Expired,
}

/// A one-time password reset link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordReset {
    /// Identifier.
    pub id: PasswordResetId,
    /// The user the reset is for.
    pub user_id: UserId,
    /// Hash of the link token (shown once to the issuing admin).
    pub token_hash: TokenHash,
    /// The issuing admin.
    pub created_by: UserId,
    /// When issued.
    pub created_at: OffsetDateTime,
    /// When it stops working (24 hours later).
    pub expires_at: OffsetDateTime,
    /// Whether using it also clears the user's MFA (BR-6.5, assumption A11).
    pub reset_mfa: bool,
    /// Set when used.
    pub used_at: Option<OffsetDateTime>,
}

impl PasswordReset {
    /// A new reset link valid for 24 hours.
    pub fn new(
        id: PasswordResetId,
        user_id: UserId,
        token_hash: TokenHash,
        created_by: UserId,
        reset_mfa: bool,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            user_id,
            token_hash,
            created_by,
            created_at: now,
            expires_at: reset_expires_at(now),
            reset_mfa,
            used_at: None,
        }
    }

    /// Check the link at `now`.
    ///
    /// # Errors
    /// [`ResetError`] when used or expired.
    pub fn check_usable(&self, now: OffsetDateTime) -> Result<(), ResetError> {
        if self.used_at.is_some() {
            return Err(ResetError::AlreadyUsed);
        }
        if now >= self.expires_at {
            return Err(ResetError::Expired);
        }
        Ok(())
    }

    /// Consume the link.
    ///
    /// # Errors
    /// [`ResetError`] when used or expired.
    pub fn mark_used(&mut self, now: OffsetDateTime) -> Result<(), ResetError> {
        self.check_usable(now)?;
        self.used_at = Some(now);
        Ok(())
    }

    /// Invalidate an unused link (a newer one was issued, BR-6.3). Used links are left alone.
    pub fn invalidate(&mut self, now: OffsetDateTime) {
        if self.used_at.is_none() {
            self.used_at = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::hash_token;
    use tandem_support::{FakeClock, FakeRng};
    use time::Duration;

    fn reset(now: OffsetDateTime) -> PasswordReset {
        let clock = FakeClock::new(now);
        let rng = FakeRng::new(9);
        PasswordReset::new(
            PasswordResetId::generate(&clock, &rng).unwrap(),
            UserId::generate(&clock, &rng).unwrap(),
            hash_token("r"),
            UserId::generate(&clock, &rng).unwrap(),
            true,
            now,
        )
    }

    #[test]
    fn br_6_1_a_reset_link_works_for_24_hours() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let r = reset(now);
        assert_eq!(
            r.check_usable(now + Duration::hours(24) - Duration::seconds(1)),
            Ok(())
        );
        assert_eq!(
            r.check_usable(now + Duration::hours(24)),
            Err(ResetError::Expired)
        );
    }

    #[test]
    fn br_6_1_a_reset_link_is_single_use() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut r = reset(now);
        r.mark_used(now).unwrap();
        assert_eq!(r.mark_used(now), Err(ResetError::AlreadyUsed));
    }

    #[test]
    fn br_6_3_issuing_a_new_link_invalidates_earlier_unused_ones() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut old = reset(now);
        old.invalidate(now);
        assert_eq!(old.check_usable(now), Err(ResetError::AlreadyUsed));
    }
}
