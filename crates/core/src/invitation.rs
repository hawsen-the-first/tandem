//! Invitations (BR-5).

use crate::email::Email;
use crate::expiry::invitation_expires_at;
use crate::ids::{InvitationId, UserId};
use crate::role::Role;
use crate::token::TokenHash;
use time::OffsetDateTime;

/// Where an invitation stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationStatus {
    /// Can still be accepted.
    Pending,
    /// Accepted; final.
    Accepted,
    /// Revoked by an admin; final.
    Revoked,
    /// Past its expiry; final.
    Expired,
}

impl InvitationStatus {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Pending, Self::Accepted, Self::Revoked, Self::Expired]
            .into_iter()
            .find(|status| status.as_str() == text)
    }
}

/// Why an invitation change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this invitation is {0:?} and cannot be changed")]
pub struct InvitationError(pub InvitationStatus);

/// An invitation to join the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    /// Identifier.
    pub id: InvitationId,
    /// The invited address.
    pub email: Email,
    /// The role applied on acceptance.
    pub role: Role,
    /// Hash of the link token (shown once to the admin, BR-5.3).
    pub token_hash: TokenHash,
    /// The issuing admin.
    pub created_by: UserId,
    /// When issued.
    pub created_at: OffsetDateTime,
    /// When it stops working.
    pub expires_at: OffsetDateTime,
    /// Stored status.
    pub status: InvitationStatus,
    /// Set on acceptance.
    pub accepted_user_id: Option<UserId>,
}

impl Invitation {
    /// A new pending invitation expiring in 7 days.
    pub fn new(
        id: InvitationId,
        email: Email,
        role: Role,
        token_hash: TokenHash,
        created_by: UserId,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            email,
            role,
            token_hash,
            created_by,
            created_at: now,
            expires_at: invitation_expires_at(now),
            status: InvitationStatus::Pending,
            accepted_user_id: None,
        }
    }

    /// The status as of `now` (a pending invitation past its expiry is expired).
    pub fn effective_status(&self, now: OffsetDateTime) -> InvitationStatus {
        if self.status == InvitationStatus::Pending && now >= self.expires_at {
            InvitationStatus::Expired
        } else {
            self.status
        }
    }

    /// Whether the link can still be used.
    pub fn is_usable(&self, now: OffsetDateTime) -> bool {
        self.effective_status(now) == InvitationStatus::Pending
    }

    /// Accept the invitation.
    ///
    /// # Errors
    /// [`InvitationError`] if it is not pending.
    pub fn accept(&mut self, user_id: UserId, now: OffsetDateTime) -> Result<(), InvitationError> {
        let status = self.effective_status(now);
        if status != InvitationStatus::Pending {
            return Err(InvitationError(status));
        }
        self.status = InvitationStatus::Accepted;
        self.accepted_user_id = Some(user_id);
        Ok(())
    }

    /// Revoke the invitation.
    ///
    /// # Errors
    /// [`InvitationError`] if it is not pending.
    pub fn revoke(&mut self, now: OffsetDateTime) -> Result<(), InvitationError> {
        let status = self.effective_status(now);
        if status != InvitationStatus::Pending {
            return Err(InvitationError(status));
        }
        self.status = InvitationStatus::Revoked;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::hash_token;
    use tandem_support::{FakeClock, FakeRng};
    use time::Duration;

    fn invitation(now: OffsetDateTime) -> Invitation {
        let clock = FakeClock::new(now);
        let rng = FakeRng::new(5);
        Invitation::new(
            InvitationId::generate(&clock, &rng).unwrap(),
            Email::parse("new@example.com").unwrap(),
            Role::Member,
            hash_token("x"),
            UserId::generate(&clock, &rng).unwrap(),
            now,
        )
    }

    #[test]
    fn br_5_2_an_invitation_expires_after_seven_days() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let inv = invitation(now);
        assert!(inv.is_usable(now + Duration::days(7) - Duration::seconds(1)));
        assert_eq!(
            inv.effective_status(now + Duration::days(7)),
            InvitationStatus::Expired
        );
    }

    #[test]
    fn br_5_5_acceptance_is_single_use() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut inv = invitation(now);
        let user = inv.created_by;
        inv.accept(user, now).unwrap();
        assert_eq!(inv.status, InvitationStatus::Accepted);
        assert_eq!(
            inv.accept(user, now),
            Err(InvitationError(InvitationStatus::Accepted))
        );
    }

    #[test]
    fn br_5_6_a_revoked_invitation_behaves_like_an_invalid_link() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut inv = invitation(now);
        inv.revoke(now).unwrap();
        assert!(!inv.is_usable(now));
        assert!(inv.accept(inv.created_by, now).is_err());
        assert_eq!(
            inv.revoke(now),
            Err(InvitationError(InvitationStatus::Revoked))
        );
    }

    #[test]
    fn br_5_4_an_expired_invitation_cannot_be_accepted() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut inv = invitation(now);
        let later = now + Duration::days(8);
        assert_eq!(
            inv.accept(inv.created_by, later),
            Err(InvitationError(InvitationStatus::Expired))
        );
    }

    #[test]
    fn status_text_forms_round_trip() {
        for status in [
            InvitationStatus::Pending,
            InvitationStatus::Accepted,
            InvitationStatus::Revoked,
            InvitationStatus::Expired,
        ] {
            assert_eq!(InvitationStatus::parse(status.as_str()), Some(status));
        }
    }
}
