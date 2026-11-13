//! The permission matrix (BR-9.3, BR-9.2, BR-4.1).

use crate::role::Role;
use crate::session::Scope;

/// Something a caller may try to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Create the first admin account.
    FirstRunSetup,
    /// Accept an invitation with a valid link.
    AcceptInvitation,
    /// Use a password reset link.
    UseResetLink,
    /// Sign in.
    SignIn,
    /// Sign out.
    SignOut,
    /// Look up the current user.
    CurrentUser,
    /// Change one's own password.
    ChangeOwnPassword,
    /// Enrol, regenerate or disable one's own MFA.
    ManageOwnMfa,
    /// Create and use skeleton chat sessions.
    UseChat,
    /// Invite, revoke, deactivate, reactivate, change roles, issue reset links.
    ManageUsers,
    /// Edit instance security settings.
    EditSettings,
    /// Configure providers, defaults, search, connectors and the sandbox policy, and run their tests (U2).
    ManageIntegrations,
    /// View the audit log.
    ViewAudit,
}

/// Who is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Principal {
    /// Not signed in.
    Anonymous,
    /// Signed in.
    Authenticated {
        /// The caller's role, read from the stored user on every request (BR-9.1).
        role: Role,
        /// The sign-in's scope.
        scope: Scope,
    },
}

/// The answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Allowed.
    Allow,
    /// Forbidden; the caller learns the resource exists.
    Deny,
    /// Behaves as if the resource does not exist (BR-1.1, BR-9.2).
    NotFound,
}

/// Evaluate the matrix. `first_run_available` is true while no user exists.
pub fn evaluate(principal: Principal, action: Action, first_run_available: bool) -> Verdict {
    use Action::{
        AcceptInvitation, ChangeOwnPassword, CurrentUser, EditSettings, FirstRunSetup,
        ManageIntegrations, ManageOwnMfa, ManageUsers, SignIn, SignOut, UseChat, UseResetLink,
        ViewAudit,
    };
    match (principal, action) {
        (_, FirstRunSetup) => {
            if first_run_available && principal == Principal::Anonymous {
                Verdict::Allow
            } else {
                Verdict::NotFound
            }
        }
        (_, AcceptInvitation | UseResetLink | SignIn) => Verdict::Allow,
        (Principal::Anonymous, _) => Verdict::Deny,
        (Principal::Authenticated { .. }, SignOut | CurrentUser | ManageOwnMfa) => Verdict::Allow,
        (Principal::Authenticated { scope, .. }, ChangeOwnPassword | UseChat) => {
            if scope == Scope::Full {
                Verdict::Allow
            } else {
                Verdict::Deny
            }
        }
        (
            Principal::Authenticated { role, scope },
            ManageUsers | EditSettings | ViewAudit | ManageIntegrations,
        ) => match (role, scope) {
            (Role::Member, _) => Verdict::NotFound,
            (Role::Admin, Scope::Full) => Verdict::Allow,
            (Role::Admin, Scope::MfaEnrolmentOnly) => Verdict::Deny,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADMIN: Principal = Principal::Authenticated {
        role: Role::Admin,
        scope: Scope::Full,
    };
    const MEMBER: Principal = Principal::Authenticated {
        role: Role::Member,
        scope: Scope::Full,
    };
    const ENROLMENT_ONLY: Principal = Principal::Authenticated {
        role: Role::Admin,
        scope: Scope::MfaEnrolmentOnly,
    };

    #[test]
    fn br_1_1_first_run_setup_exists_only_for_anonymous_callers_while_no_user_exists() {
        assert_eq!(
            evaluate(Principal::Anonymous, Action::FirstRunSetup, true),
            Verdict::Allow
        );
        assert_eq!(
            evaluate(Principal::Anonymous, Action::FirstRunSetup, false),
            Verdict::NotFound
        );
        assert_eq!(
            evaluate(ADMIN, Action::FirstRunSetup, true),
            Verdict::NotFound
        );
    }

    #[test]
    fn br_9_3_public_entry_points_are_open_to_anyone() {
        for action in [
            Action::SignIn,
            Action::AcceptInvitation,
            Action::UseResetLink,
        ] {
            assert_eq!(
                evaluate(Principal::Anonymous, action, false),
                Verdict::Allow
            );
        }
    }

    #[test]
    fn br_9_3_signed_in_actions_need_a_session() {
        for action in [
            Action::SignOut,
            Action::CurrentUser,
            Action::ManageOwnMfa,
            Action::ChangeOwnPassword,
            Action::UseChat,
            Action::ManageUsers,
        ] {
            assert_eq!(evaluate(Principal::Anonymous, action, false), Verdict::Deny);
        }
    }

    #[test]
    fn br_9_2_admin_resources_are_not_found_for_members() {
        for action in [Action::ManageUsers, Action::EditSettings, Action::ViewAudit] {
            assert_eq!(evaluate(MEMBER, action, false), Verdict::NotFound);
            assert_eq!(evaluate(ADMIN, action, false), Verdict::Allow);
        }
    }

    #[test]
    fn br_4_1_an_enrolment_only_session_can_only_manage_mfa_sign_out_and_look_itself_up() {
        for action in [Action::ManageOwnMfa, Action::SignOut, Action::CurrentUser] {
            assert_eq!(evaluate(ENROLMENT_ONLY, action, false), Verdict::Allow);
        }
        for action in [
            Action::ChangeOwnPassword,
            Action::UseChat,
            Action::ManageUsers,
            Action::EditSettings,
            Action::ViewAudit,
        ] {
            assert_eq!(evaluate(ENROLMENT_ONLY, action, false), Verdict::Deny);
        }
    }

    #[test]
    fn br_9_3_members_can_change_their_password_and_use_chat() {
        assert_eq!(
            evaluate(MEMBER, Action::ChangeOwnPassword, false),
            Verdict::Allow
        );
        assert_eq!(evaluate(MEMBER, Action::UseChat, false), Verdict::Allow);
    }
}

#[cfg(test)]
mod integrations_tests {
    use super::*;

    #[test]
    fn br_21_3_managing_integrations_is_for_admins_with_a_full_scope_sign_in_only() {
        let admin = Principal::Authenticated {
            role: Role::Admin,
            scope: Scope::Full,
        };
        let member = Principal::Authenticated {
            role: Role::Member,
            scope: Scope::Full,
        };
        let enrolling = Principal::Authenticated {
            role: Role::Admin,
            scope: Scope::MfaEnrolmentOnly,
        };
        assert_eq!(
            evaluate(admin, Action::ManageIntegrations, false),
            Verdict::Allow
        );
        assert_eq!(
            evaluate(member, Action::ManageIntegrations, false),
            Verdict::NotFound
        );
        assert_eq!(
            evaluate(enrolling, Action::ManageIntegrations, false),
            Verdict::Deny
        );
        assert_eq!(
            evaluate(Principal::Anonymous, Action::ManageIntegrations, false),
            Verdict::Deny
        );
    }
}
