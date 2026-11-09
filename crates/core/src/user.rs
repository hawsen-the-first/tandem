//! Users (BR-1, BR-7).

use crate::display_name::DisplayName;
use crate::email::Email;
use crate::ids::UserId;
use crate::role::{Role, UserStatus};
use time::OffsetDateTime;

/// How an account came to exist (BR-1.5: there is no self-registration).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatedVia {
    /// The first-run setup screen.
    FirstRunSetup,
    /// An accepted invitation.
    Invitation,
}

impl CreatedVia {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstRunSetup => "first_run_setup",
            Self::Invitation => "invitation",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "first_run_setup" => Some(Self::FirstRunSetup),
            "invitation" => Some(Self::Invitation),
            _ => None,
        }
    }
}

/// Why a user change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UserError {
    /// Already deactivated.
    #[error("the user is already deactivated")]
    AlreadyDeactivated,
    /// Already active.
    #[error("the user is already active")]
    AlreadyActive,
    /// The role would not change.
    #[error("the user already has that role")]
    RoleUnchanged,
}

/// A person who can sign in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Identifier.
    pub id: UserId,
    /// Sign-in identifier.
    pub email: Email,
    /// Shown to other members.
    pub display_name: DisplayName,
    /// Admin or member.
    pub role: Role,
    /// Active or deactivated.
    pub status: UserStatus,
    /// The stored password hash (never exposed).
    pub password_hash: String,
    /// When the password last changed.
    pub password_changed_at: OffsetDateTime,
    /// When the account was created.
    pub created_at: OffsetDateTime,
    /// How the account was created.
    pub created_via: CreatedVia,
    /// When it was deactivated, while it is.
    pub deactivated_at: Option<OffsetDateTime>,
    /// Who deactivated it, while it is.
    pub deactivated_by: Option<UserId>,
    /// The latest successful sign-in.
    pub last_sign_in_at: Option<OffsetDateTime>,
}

impl User {
    /// A deactivated account cannot sign in (BR-1.4).
    pub fn can_sign_in(&self) -> bool {
        self.status == UserStatus::Active
    }

    /// Whether this user counts as an active admin (BR-7.2).
    pub fn is_active_admin(&self) -> bool {
        self.role == Role::Admin && self.status == UserStatus::Active
    }

    /// Deactivate the account.
    ///
    /// # Errors
    /// [`UserError::AlreadyDeactivated`] if it already is.
    pub fn deactivate(&mut self, by: UserId, now: OffsetDateTime) -> Result<(), UserError> {
        if self.status == UserStatus::Deactivated {
            return Err(UserError::AlreadyDeactivated);
        }
        self.status = UserStatus::Deactivated;
        self.deactivated_at = Some(now);
        self.deactivated_by = Some(by);
        Ok(())
    }

    /// Reactivate the account (existing password and MFA are kept, BR-7.4).
    ///
    /// # Errors
    /// [`UserError::AlreadyActive`] if it already is.
    pub fn reactivate(&mut self) -> Result<(), UserError> {
        if self.status == UserStatus::Active {
            return Err(UserError::AlreadyActive);
        }
        self.status = UserStatus::Active;
        self.deactivated_at = None;
        self.deactivated_by = None;
        Ok(())
    }

    /// Change the role.
    ///
    /// # Errors
    /// [`UserError::RoleUnchanged`] if the role is already `role`.
    pub fn change_role(&mut self, role: Role) -> Result<(), UserError> {
        if self.role == role {
            return Err(UserError::RoleUnchanged);
        }
        self.role = role;
        Ok(())
    }

    /// Store a new password hash.
    pub fn set_password_hash(&mut self, hash: String, now: OffsetDateTime) {
        self.password_hash = hash;
        self.password_changed_at = now;
    }

    /// Record a successful sign-in.
    pub fn record_sign_in(&mut self, now: OffsetDateTime) {
        self.last_sign_in_at = Some(now);
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    pub fn user(role: Role) -> User {
        let clock = FakeClock::at_unix(1_700_000_000);
        let rng = FakeRng::new(42);
        User {
            id: UserId::generate(&clock, &rng).unwrap(),
            email: Email::parse("ana@example.com").unwrap(),
            display_name: DisplayName::parse("Ana").unwrap(),
            role,
            status: UserStatus::Active,
            password_hash: "hash".into(),
            password_changed_at: clock.now_for_tests(),
            created_at: clock.now_for_tests(),
            created_via: CreatedVia::Invitation,
            deactivated_at: None,
            deactivated_by: None,
            last_sign_in_at: None,
        }
    }

    trait ClockExt {
        fn now_for_tests(&self) -> OffsetDateTime;
    }
    impl ClockExt for FakeClock {
        fn now_for_tests(&self) -> OffsetDateTime {
            tandem_support::Clock::now(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::user;
    use super::*;

    #[test]
    fn br_1_4_a_deactivated_user_cannot_sign_in_and_reactivation_restores_it() {
        let mut member = user(Role::Member);
        let admin = user(Role::Admin).id;
        assert!(member.can_sign_in());
        member
            .deactivate(admin, OffsetDateTime::UNIX_EPOCH)
            .unwrap();
        assert!(!member.can_sign_in());
        assert_eq!(member.deactivated_by, Some(admin));
        assert_eq!(
            member.deactivate(admin, OffsetDateTime::UNIX_EPOCH),
            Err(UserError::AlreadyDeactivated)
        );
        member.reactivate().unwrap();
        assert!(member.can_sign_in());
        assert_eq!(member.deactivated_at, None);
        assert_eq!(member.reactivate(), Err(UserError::AlreadyActive));
    }

    #[test]
    fn br_7_4_reactivation_keeps_the_password() {
        let mut member = user(Role::Member);
        member.set_password_hash("stored".into(), OffsetDateTime::UNIX_EPOCH);
        member
            .deactivate(user(Role::Admin).id, OffsetDateTime::UNIX_EPOCH)
            .unwrap();
        member.reactivate().unwrap();
        assert_eq!(member.password_hash, "stored");
    }

    #[test]
    fn br_7_1_roles_change_between_admin_and_member_only() {
        let mut member = user(Role::Member);
        member.change_role(Role::Admin).unwrap();
        assert!(member.is_active_admin());
        assert_eq!(
            member.change_role(Role::Admin),
            Err(UserError::RoleUnchanged)
        );
    }

    #[test]
    fn br_1_5_creation_paths_are_only_setup_and_invitation() {
        assert_eq!(
            CreatedVia::parse("first_run_setup"),
            Some(CreatedVia::FirstRunSetup)
        );
        assert_eq!(
            CreatedVia::parse("invitation"),
            Some(CreatedVia::Invitation)
        );
        assert_eq!(CreatedVia::parse("self_registration"), None);
    }
}
