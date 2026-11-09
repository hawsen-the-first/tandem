//! Roles and account status.

/// Instance role (FR-1.4): there are exactly two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Can administer users, settings and the audit log.
    Admin,
    /// An ordinary team member.
    Member,
}

impl Role {
    /// Stable text form used in storage and the API.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "admin" => Some(Self::Admin),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

/// Account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStatus {
    /// Can sign in.
    Active,
    /// Cannot sign in; reversible (BR-7.4).
    Deactivated,
}

impl UserStatus {
    /// Stable text form used in storage and the API.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deactivated => "deactivated",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "active" => Some(Self::Active),
            "deactivated" => Some(Self::Deactivated),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_forms_round_trip() {
        for role in [Role::Admin, Role::Member] {
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        for status in [UserStatus::Active, UserStatus::Deactivated] {
            assert_eq!(UserStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(Role::parse("owner"), None);
    }
}
