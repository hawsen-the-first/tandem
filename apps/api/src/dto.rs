//! Request and response bodies. Domain types are never serialised directly (they hold password
//! hashes and token hashes); each response type names exactly what the client may see.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tandem_app::{InvitationView, MfaStatus, UserView};
use tandem_core::audit::AuditEntry;
use tandem_core::chat::{Author, ChatMessage, ChatSession};
use tandem_core::settings::SecuritySettings;
use tandem_core::user::User;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

/// RFC 3339 text for a timestamp.
pub fn timestamp(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}

/// A user as the client sees them.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserDto {
    /// The user's ID.
    pub id: String,
    /// Sign-in email.
    pub email: String,
    /// Display name.
    pub display_name: String,
    /// `admin` or `member`.
    pub role: String,
    /// `active` or `deactivated`.
    pub status: String,
    /// When the account was created.
    pub created_at: String,
    /// The last successful sign-in, if any.
    pub last_sign_in_at: Option<String>,
}

impl From<&User> for UserDto {
    fn from(user: &User) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.as_str().to_owned(),
            display_name: user.display_name.as_str().to_owned(),
            role: user.role.as_str().to_owned(),
            status: user.status.as_str().to_owned(),
            created_at: timestamp(user.created_at),
            last_sign_in_at: user.last_sign_in_at.map(timestamp),
        }
    }
}

/// A user in the admin list, with their MFA state.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUserDto {
    /// The user.
    #[serde(flatten)]
    pub user: UserDto,
    /// Whether the user has confirmed MFA.
    pub mfa_enrolled: bool,
}

impl From<&UserView> for AdminUserDto {
    fn from(view: &UserView) -> Self {
        Self {
            user: UserDto::from(&view.user),
            mfa_enrolled: view.mfa_enrolled,
        }
    }
}

/// MFA state.
#[derive(Debug, Serialize, ToSchema)]
pub struct MfaStatusDto {
    /// MFA is confirmed.
    pub enrolled: bool,
    /// Unused recovery codes left.
    pub recovery_codes_remaining: usize,
    /// Suggest generating a new set.
    pub remind_regenerate: bool,
}

impl From<MfaStatus> for MfaStatusDto {
    fn from(status: MfaStatus) -> Self {
        Self {
            enrolled: status.enrolled,
            recovery_codes_remaining: status.recovery_codes_remaining,
            remind_regenerate: status.remind_regenerate,
        }
    }
}

/// The signed-in user, their scope and the CSRF token for state-changing calls.
#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    /// The user.
    pub user: UserDto,
    /// `full` or `mfa_enrolment_only`.
    pub scope: String,
    /// MFA state.
    pub mfa: MfaStatusDto,
    /// Send as `X-CSRF-Token` on POST, PUT, PATCH and DELETE.
    pub csrf_token: String,
}

/// Whether first-run setup is still open.
#[derive(Debug, Serialize, ToSchema)]
pub struct SetupStatus {
    /// True until the first admin exists.
    pub available: bool,
}

/// First-run setup.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetupRequest {
    /// The admin's display name.
    pub display_name: String,
    /// The admin's email.
    pub email: String,
    /// The admin's password.
    pub password: String,
}

/// A second factor submitted with sign-in.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SecondFactorDto {
    /// `totp` or `recovery`.
    pub kind: String,
    /// The code.
    pub code: String,
}

/// Sign-in.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SignInRequestDto {
    /// Email as typed.
    pub email: String,
    /// Password.
    pub password: String,
    /// Required when the account has MFA.
    pub second_factor: Option<SecondFactorDto>,
}

/// The result of a sign-in step.
#[derive(Debug, Serialize, ToSchema)]
pub struct SignInResponse {
    /// `signed_in` or `mfa_required`.
    pub status: String,
    /// The user, when signed in.
    pub user: Option<UserDto>,
    /// `full` or `mfa_enrolment_only`, when signed in.
    pub scope: Option<String>,
}

/// A session started by setup or invitation acceptance.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionStarted {
    /// The user.
    pub user: UserDto,
    /// `full` or `mfa_enrolment_only`.
    pub scope: String,
}

/// The MFA secret to show once.
#[derive(Debug, Serialize, ToSchema)]
pub struct EnrolmentResponse {
    /// The secret for manual entry.
    pub secret_base32: String,
    /// The `otpauth://` URI for a QR code.
    pub otpauth_uri: String,
}

/// A code to confirm enrolment.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeRequest {
    /// A 6-digit authenticator code.
    pub code: String,
}

/// Recovery codes, shown once.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecoveryCodesResponse {
    /// The codes.
    pub recovery_codes: Vec<String>,
}

/// Turn MFA off (members only).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DisableMfaRequest {
    /// The current password.
    pub password: String,
    /// A current authenticator or recovery code.
    pub code: String,
}

/// Change the signed-in user's password.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    /// The current password.
    pub current_password: String,
    /// The new password.
    pub new_password: String,
}

/// A link token from an invitation or reset link.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    /// The token from the link.
    pub token: String,
}

/// What an invitation link is for.
#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationInfoDto {
    /// The invited email.
    pub email: String,
    /// The role being offered.
    pub role: String,
}

/// Accept an invitation.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitationRequest {
    /// The token from the link.
    pub token: String,
    /// The new user's display name.
    pub display_name: String,
    /// The new user's password.
    pub password: String,
}

/// Whose password a reset link is for.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResetInfoDto {
    /// The user's display name.
    pub display_name: String,
}

/// Finish a password reset.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteResetRequest {
    /// The token from the link.
    pub token: String,
    /// The new password.
    pub new_password: String,
}

/// An invitation in the admin list.
#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationDto {
    /// The invitation ID.
    pub id: String,
    /// Invited email.
    pub email: String,
    /// Offered role.
    pub role: String,
    /// `pending`, `accepted`, `revoked` or `expired`.
    pub status: String,
    /// Created at.
    pub created_at: String,
    /// Expires at.
    pub expires_at: String,
}

impl From<&InvitationView> for InvitationDto {
    fn from(view: &InvitationView) -> Self {
        Self {
            id: view.invitation.id.to_string(),
            email: view.invitation.email.as_str().to_owned(),
            role: view.invitation.role.as_str().to_owned(),
            status: view.status.as_str().to_owned(),
            created_at: timestamp(view.invitation.created_at),
            expires_at: timestamp(view.invitation.expires_at),
        }
    }
}

/// Create an invitation.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteRequest {
    /// Who to invite.
    pub email: String,
    /// `admin` or `member`.
    pub role: String,
}

/// A new invitation with its one-time link.
#[derive(Debug, Serialize, ToSchema)]
pub struct InviteCreatedDto {
    /// The invitation.
    pub invitation: InvitationDto,
    /// The link to give to the invitee; shown once.
    pub link: String,
}

/// Change a user's role.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleRequest {
    /// `admin` or `member`.
    pub role: String,
}

/// Issue a reset link.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetLinkRequest {
    /// Also remove the user's MFA enrolment.
    #[serde(default)]
    pub reset_mfa: bool,
}

/// A reset link, shown once.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResetLinkDto {
    /// The link to give to the user.
    pub link: String,
    /// When it stops working.
    pub expires_at: String,
    /// Whose password it resets.
    pub user_display_name: String,
}

/// The instance security settings.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SecuritySettingsDto {
    /// Sign out after this many idle seconds.
    pub idle_timeout_seconds: i64,
    /// Sign out after this many seconds in total.
    pub absolute_duration_seconds: i64,
    /// Also check passwords against the online breach service.
    pub online_breach_check: bool,
}

impl From<SecuritySettings> for SecuritySettingsDto {
    fn from(settings: SecuritySettings) -> Self {
        Self {
            idle_timeout_seconds: settings.idle_timeout.whole_seconds(),
            absolute_duration_seconds: settings.absolute_duration.whole_seconds(),
            online_breach_check: settings.online_breach_check,
        }
    }
}

impl SecuritySettingsDto {
    /// The domain value (validated by the service).
    pub fn into_settings(self) -> SecuritySettings {
        SecuritySettings {
            idle_timeout: time::Duration::seconds(self.idle_timeout_seconds),
            absolute_duration: time::Duration::seconds(self.absolute_duration_seconds),
            online_breach_check: self.online_breach_check,
        }
    }
}

/// An audit entry.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEntryDto {
    /// The entry ID.
    pub id: String,
    /// When it happened.
    pub occurred_at: String,
    /// The acting user, if any.
    pub actor_id: Option<String>,
    /// The actor's name at the time.
    pub actor_label: String,
    /// The event type, for example `auth.sign_in.failure`.
    pub event_type: String,
    /// What it concerned.
    pub object_type: Option<String>,
    /// Which one.
    pub object_id: Option<String>,
    /// `success`, `failure` or `denied`.
    pub outcome: String,
    /// Extra facts, with secrets already removed.
    pub details: BTreeMap<String, String>,
    /// The request it came from.
    pub request_id: Option<String>,
    /// The source address.
    pub source: Option<String>,
}

impl From<&AuditEntry> for AuditEntryDto {
    fn from(entry: &AuditEntry) -> Self {
        let event = &entry.event;
        Self {
            id: entry.id.to_string(),
            occurred_at: timestamp(event.occurred_at),
            actor_id: event.actor_id.map(|id| id.to_string()),
            actor_label: event.actor_label.clone(),
            event_type: event.event_type.as_str().to_owned(),
            object_type: event.object_type.clone(),
            object_id: event.object_id.clone(),
            outcome: event.outcome.as_str().to_owned(),
            details: event.details.clone(),
            request_id: event.request_id.clone(),
            source: event.source.clone(),
        }
    }
}

/// A page of audit entries, newest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditPageDto {
    /// The entries.
    pub entries: Vec<AuditEntryDto>,
    /// Pass these two values back as `after_time` and `after_id` for the next page.
    pub next_after_time: Option<String>,
    /// See `next_after_time`.
    pub next_after_id: Option<String>,
}

/// A chat session.
#[derive(Debug, Serialize, ToSchema)]
pub struct ChatSessionDto {
    /// The session ID.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Created at.
    pub created_at: String,
}

impl From<&ChatSession> for ChatSessionDto {
    fn from(session: &ChatSession) -> Self {
        Self {
            id: session.id.to_string(),
            title: session.title.clone(),
            created_at: timestamp(session.created_at),
        }
    }
}

/// Create a chat session.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateChatSessionRequest {
    /// The title.
    pub title: String,
}

/// A chat message.
#[derive(Debug, Serialize, ToSchema)]
pub struct ChatMessageDto {
    /// The message ID.
    pub id: String,
    /// The session it belongs to.
    pub chat_session_id: String,
    /// `user` or `agent`.
    pub author_kind: String,
    /// The user ID or the agent's name.
    pub author: String,
    /// The text.
    pub body: String,
    /// Posted at.
    pub created_at: String,
    /// The message this answers, for agent replies.
    pub in_reply_to: Option<String>,
}

impl From<&ChatMessage> for ChatMessageDto {
    fn from(message: &ChatMessage) -> Self {
        let (author_kind, author) = match &message.author {
            Author::User(id) => ("user", id.to_string()),
            Author::Agent(name) => ("agent", name.clone()),
        };
        Self {
            id: message.id.to_string(),
            chat_session_id: message.chat_session_id.to_string(),
            author_kind: author_kind.to_owned(),
            author,
            body: message.body.clone(),
            created_at: timestamp(message.created_at),
            in_reply_to: message.in_reply_to.map(|id| id.to_string()),
        }
    }
}

/// Post a chat message.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PostMessageRequest {
    /// The text.
    pub body: String,
}

/// The legal notices and provenance of this running copy.
#[derive(Debug, Serialize, ToSchema)]
pub struct AboutDto {
    /// The product name.
    pub name: String,
    /// The version of this build.
    pub version: String,
    /// The copyright line.
    pub copyright: String,
    /// The licence, as an SPDX identifier.
    pub license: String,
    /// Where to read the licence text.
    pub license_url: String,
    /// Where the Corresponding Source of this running version is (AGPL section 13).
    pub source_url: String,
    /// The original project, credited as "Based on Tandem".
    pub upstream_url: String,
}
