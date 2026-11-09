//! Rules that protect the instance from losing its administrators (BR-7.2, BR-7.3).

use crate::ids::UserId;
use crate::user::User;

/// Why an administrative change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdminRuleError {
    /// The change would leave no active admin.
    #[error("at least one active admin is required")]
    LastAdmin,
    /// An admin cannot deactivate themselves.
    #[error("you cannot deactivate your own account; ask another admin")]
    SelfDeactivation,
}

/// The last active admin cannot be deactivated or demoted (BR-7.2).
///
/// `active_admin_count` must be read inside the same transaction as the change.
///
/// # Errors
/// [`AdminRuleError::LastAdmin`] when `target` is the only active admin.
pub fn check_can_lose_admin(target: &User, active_admin_count: u64) -> Result<(), AdminRuleError> {
    if target.is_active_admin() && active_admin_count <= 1 {
        Err(AdminRuleError::LastAdmin)
    } else {
        Ok(())
    }
}

/// An admin cannot deactivate themselves (BR-7.3).
///
/// # Errors
/// [`AdminRuleError::SelfDeactivation`] when actor and target are the same.
pub fn check_not_self_deactivation(actor: UserId, target: UserId) -> Result<(), AdminRuleError> {
    if actor == target {
        Err(AdminRuleError::SelfDeactivation)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::Role;
    use crate::user::fixtures::user;

    #[test]
    fn br_7_2_the_last_active_admin_cannot_be_deactivated_or_demoted() {
        let admin = user(Role::Admin);
        assert_eq!(
            check_can_lose_admin(&admin, 1),
            Err(AdminRuleError::LastAdmin)
        );
        assert_eq!(
            check_can_lose_admin(&admin, 0),
            Err(AdminRuleError::LastAdmin)
        );
        assert_eq!(check_can_lose_admin(&admin, 2), Ok(()));
    }

    #[test]
    fn br_7_2_members_and_inactive_admins_are_not_protected() {
        assert_eq!(check_can_lose_admin(&user(Role::Member), 1), Ok(()));
        let mut inactive = user(Role::Admin);
        inactive.status = crate::role::UserStatus::Deactivated;
        assert_eq!(check_can_lose_admin(&inactive, 1), Ok(()));
    }

    #[test]
    fn br_7_3_nobody_deactivates_themselves() {
        let admin = user(Role::Admin);
        assert_eq!(
            check_not_self_deactivation(admin.id, admin.id),
            Err(AdminRuleError::SelfDeactivation)
        );
        let other = user(Role::Member);
        // Fixture users share an id; build a different one.
        let different = UserId::from_uuid(uuid::Uuid::from_u128(other.id.as_uuid().as_u128() ^ 1));
        assert_eq!(check_not_self_deactivation(admin.id, different), Ok(()));
    }
}
