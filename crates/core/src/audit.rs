//! Audit events (BR-11): a fixed catalogue, no secrets in details.

use crate::ids::{AuditEntryId, UserId};
use std::collections::BTreeMap;
use time::OffsetDateTime;

/// Detail keys containing any of these fragments are dropped (BR-11.4).
const FORBIDDEN_DETAIL_FRAGMENTS: [&str; 6] = [
    "password",
    "token",
    "secret",
    "code",
    "cookie",
    "authorization",
];

/// Every event the audit log can contain (BR-11.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AuditEventType {
    /// First-run admin created.
    AuthSetup,
    /// Successful sign-in.
    AuthSignInSuccess,
    /// Failed sign-in for an existing account.
    AuthSignInFailure,
    /// An account was locked.
    AuthSignInLocked,
    /// A user signed out.
    AuthSignOut,
    /// MFA enrolment completed.
    AuthMfaEnrolled,
    /// A member switched MFA off.
    AuthMfaDisabled,
    /// A recovery code was used.
    AuthMfaRecoveryUsed,
    /// Recovery codes were regenerated.
    AuthMfaRecoveryRegenerated,
    /// A user changed their password.
    AuthPasswordChanged,
    /// An admin issued a reset link.
    AuthPasswordResetIssued,
    /// A reset link was used.
    AuthPasswordResetCompleted,
    /// An invitation was issued.
    UserInvited,
    /// An invitation was revoked.
    UserInviteRevoked,
    /// An invitation was accepted.
    UserInviteAccepted,
    /// A user was deactivated.
    UserDeactivated,
    /// A user was reactivated.
    UserReactivated,
    /// A user's role changed.
    UserRoleChanged,
    /// An instance setting changed.
    SettingsChanged,
    /// A signed-in user attempted a forbidden action.
    AuthzDenied,
    /// A model provider was added.
    ConfigProviderCreated,
    /// A model provider was changed.
    ConfigProviderUpdated,
    /// A model provider was removed.
    ConfigProviderDeleted,
    /// The default models changed.
    ConfigDefaultsUpdated,
    /// Search settings or the active search provider changed.
    ConfigSearchUpdated,
    /// A connector was configured or removed.
    ConfigConnectorUpdated,
    /// The sandbox policy changed.
    ConfigSandboxPolicyUpdated,
    /// A configuration test ran.
    ConfigTestRun,
}

impl AuditEventType {
    /// All event types, in catalogue order.
    pub const ALL: [Self; 28] = [
        Self::AuthSetup,
        Self::AuthSignInSuccess,
        Self::AuthSignInFailure,
        Self::AuthSignInLocked,
        Self::AuthSignOut,
        Self::AuthMfaEnrolled,
        Self::AuthMfaDisabled,
        Self::AuthMfaRecoveryUsed,
        Self::AuthMfaRecoveryRegenerated,
        Self::AuthPasswordChanged,
        Self::AuthPasswordResetIssued,
        Self::AuthPasswordResetCompleted,
        Self::UserInvited,
        Self::UserInviteRevoked,
        Self::UserInviteAccepted,
        Self::UserDeactivated,
        Self::UserReactivated,
        Self::UserRoleChanged,
        Self::SettingsChanged,
        Self::AuthzDenied,
        Self::ConfigProviderCreated,
        Self::ConfigProviderUpdated,
        Self::ConfigProviderDeleted,
        Self::ConfigDefaultsUpdated,
        Self::ConfigSearchUpdated,
        Self::ConfigConnectorUpdated,
        Self::ConfigSandboxPolicyUpdated,
        Self::ConfigTestRun,
    ];

    /// The stable dotted name stored and shown in the log.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthSetup => "auth.setup",
            Self::AuthSignInSuccess => "auth.sign_in.success",
            Self::AuthSignInFailure => "auth.sign_in.failure",
            Self::AuthSignInLocked => "auth.sign_in.locked",
            Self::AuthSignOut => "auth.sign_out",
            Self::AuthMfaEnrolled => "auth.mfa.enrolled",
            Self::AuthMfaDisabled => "auth.mfa.disabled",
            Self::AuthMfaRecoveryUsed => "auth.mfa.recovery_used",
            Self::AuthMfaRecoveryRegenerated => "auth.mfa.recovery_regenerated",
            Self::AuthPasswordChanged => "auth.password.changed",
            Self::AuthPasswordResetIssued => "auth.password.reset_issued",
            Self::AuthPasswordResetCompleted => "auth.password.reset_completed",
            Self::UserInvited => "user.invited",
            Self::UserInviteRevoked => "user.invite_revoked",
            Self::UserInviteAccepted => "user.invite_accepted",
            Self::UserDeactivated => "user.deactivated",
            Self::UserReactivated => "user.reactivated",
            Self::UserRoleChanged => "user.role_changed",
            Self::SettingsChanged => "settings.changed",
            Self::AuthzDenied => "authz.denied",
            Self::ConfigProviderCreated => "config.provider.created",
            Self::ConfigProviderUpdated => "config.provider.updated",
            Self::ConfigProviderDeleted => "config.provider.deleted",
            Self::ConfigDefaultsUpdated => "config.defaults.updated",
            Self::ConfigSearchUpdated => "config.search.updated",
            Self::ConfigConnectorUpdated => "config.connector.updated",
            Self::ConfigSandboxPolicyUpdated => "config.sandbox_policy.updated",
            Self::ConfigTestRun => "config.test.run",
        }
    }

    /// Parse a stored name.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == text)
    }
}

/// How the audited action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutcome {
    /// It happened.
    Success,
    /// It was attempted and failed.
    Failure,
    /// It was refused.
    Denied,
}

impl AuditOutcome {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Denied => "denied",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Success, Self::Failure, Self::Denied]
            .into_iter()
            .find(|o| o.as_str() == text)
    }
}

/// One audit event, built with the methods below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// When it happened.
    pub occurred_at: OffsetDateTime,
    /// The acting user, if any.
    pub actor_id: Option<UserId>,
    /// A snapshot of the actor's name, `system` or `unknown source`.
    pub actor_label: String,
    /// What happened.
    pub event_type: AuditEventType,
    /// The kind of object affected (for example `user`).
    pub object_type: Option<String>,
    /// The affected object's identifier.
    pub object_id: Option<String>,
    /// How it ended.
    pub outcome: AuditOutcome,
    /// Non-secret context.
    pub details: BTreeMap<String, String>,
    /// The request that caused it.
    pub request_id: Option<String>,
    /// Source address and client summary.
    pub source: Option<String>,
}

impl AuditEvent {
    /// Start an event with no actor (labelled `system`).
    pub fn new(
        event_type: AuditEventType,
        outcome: AuditOutcome,
        occurred_at: OffsetDateTime,
    ) -> Self {
        Self {
            occurred_at,
            actor_id: None,
            actor_label: "system".to_owned(),
            event_type,
            object_type: None,
            object_id: None,
            outcome,
            details: BTreeMap::new(),
            request_id: None,
            source: None,
        }
    }

    /// Set the acting user and a snapshot of their name.
    #[must_use]
    pub fn actor(mut self, id: UserId, label: impl Into<String>) -> Self {
        self.actor_id = Some(id);
        self.actor_label = label.into();
        self
    }

    /// Label an actor that is not a known user (for example an anonymous request).
    #[must_use]
    pub fn actor_label(mut self, label: impl Into<String>) -> Self {
        self.actor_label = label.into();
        self
    }

    /// Set the affected object.
    #[must_use]
    pub fn object(mut self, object_type: impl Into<String>, object_id: impl ToString) -> Self {
        self.object_type = Some(object_type.into());
        self.object_id = Some(object_id.to_string());
        self
    }

    /// Add a detail. Keys that look like secrets are silently dropped (BR-11.4).
    #[must_use]
    pub fn detail(mut self, key: &str, value: impl Into<String>) -> Self {
        let lowered = key.to_lowercase();
        if !FORBIDDEN_DETAIL_FRAGMENTS
            .iter()
            .any(|fragment| lowered.contains(fragment))
        {
            self.details.insert(key.to_owned(), value.into());
        }
        self
    }

    /// Attach the request ID.
    #[must_use]
    pub fn request(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    /// Attach the source summary.
    #[must_use]
    pub fn source(mut self, source: Option<String>) -> Self {
        self.source = source;
        self
    }
}

/// A stored audit entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Identifier.
    pub id: AuditEntryId,
    /// The event.
    pub event: AuditEvent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn br_11_5_the_catalogue_has_twenty_eight_unique_dotted_names_that_round_trip() {
        let names: HashSet<&str> = AuditEventType::ALL.iter().map(|e| e.as_str()).collect();
        assert_eq!(names.len(), 28);
        for event in AuditEventType::ALL {
            assert!(event.as_str().contains('.'));
            assert_eq!(AuditEventType::parse(event.as_str()), Some(event));
        }
        assert_eq!(
            AuditEventType::parse("audit.pruned"),
            None,
            "no automatic pruning exists"
        );
    }

    #[test]
    fn br_11_4_details_that_look_like_secrets_are_dropped() {
        let event = AuditEvent::new(
            AuditEventType::UserRoleChanged,
            AuditOutcome::Success,
            OffsetDateTime::UNIX_EPOCH,
        )
        .detail("old_role", "member")
        .detail("new_role", "admin")
        .detail("password", "hunter2")
        .detail("Reset_Token", "abc")
        .detail("recovery_code", "x")
        .detail("session_cookie", "c")
        .detail("Authorization", "Bearer x")
        .detail("client_secret", "s");
        let keys: Vec<&str> = event.details.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["new_role", "old_role"]);
    }

    #[test]
    fn events_without_an_actor_are_labelled_system_and_builders_fill_fields() {
        let event = AuditEvent::new(
            AuditEventType::AuthSetup,
            AuditOutcome::Success,
            OffsetDateTime::UNIX_EPOCH,
        )
        .object("user", "u-1")
        .request(Some("req-1".into()))
        .source(Some("203.0.113.9".into()));
        assert_eq!(event.actor_label, "system");
        assert_eq!(event.object_id.as_deref(), Some("u-1"));
        assert_eq!(event.request_id.as_deref(), Some("req-1"));
        assert_eq!(AuditOutcome::parse("denied"), Some(AuditOutcome::Denied));
    }
}
