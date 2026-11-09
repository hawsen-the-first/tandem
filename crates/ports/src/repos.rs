//! Repository traits. Every method belongs to the transaction that handed out the repository.

use crate::error::StoreError;
use async_trait::async_trait;
use std::collections::BTreeMap;
use tandem_core::audit::{AuditEntry, AuditEventType};
use tandem_core::chat::{ChatMessage, ChatSession};
use tandem_core::email::Email;
use tandem_core::ids::{
    AuditEntryId, AuthSessionId, ChatMessageId, ChatSessionId, InvitationId, JobId, OutboxEventId,
    UserId,
};
use tandem_core::invitation::Invitation;
use tandem_core::job::Job;
use tandem_core::mfa::{MfaCredential, RecoveryCode};
use tandem_core::outbox::{NewOutboxEvent, OutboxEvent};
use tandem_core::reset::PasswordReset;
use tandem_core::session::{AuthSession, EndReason};
use tandem_core::throttle::GuardState;
use tandem_core::token::TokenHash;
use tandem_core::user::User;
use time::{Duration, OffsetDateTime};

/// Users.
#[async_trait]
pub trait UserRepo: Send {
    /// Insert a user.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] (`users.email`) when the email is taken.
    async fn insert(&mut self, user: &User) -> Result<(), StoreError>;
    /// Fetch by id.
    async fn get(&mut self, id: UserId) -> Result<Option<User>, StoreError>;
    /// Fetch by normalised email.
    async fn find_by_email(&mut self, email: &Email) -> Result<Option<User>, StoreError>;
    /// All users, oldest first.
    async fn list(&mut self) -> Result<Vec<User>, StoreError>;
    /// Save changes to an existing user.
    async fn update(&mut self, user: &User) -> Result<(), StoreError>;
    /// How many users exist.
    async fn count_all(&mut self) -> Result<u64, StoreError>;
    /// How many active admins exist.
    async fn count_active_admins(&mut self) -> Result<u64, StoreError>;
}

/// Invitations.
#[async_trait]
pub trait InvitationRepo: Send {
    /// Insert an invitation.
    async fn insert(&mut self, invitation: &Invitation) -> Result<(), StoreError>;
    /// Fetch by id.
    async fn get(&mut self, id: InvitationId) -> Result<Option<Invitation>, StoreError>;
    /// Fetch by the hash of its link token.
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<Invitation>, StoreError>;
    /// A pending, unexpired invitation for the email, if any.
    async fn find_pending_by_email(
        &mut self,
        email: &Email,
        now: OffsetDateTime,
    ) -> Result<Option<Invitation>, StoreError>;
    /// All invitations, newest first.
    async fn list(&mut self) -> Result<Vec<Invitation>, StoreError>;
    /// Save changes.
    async fn update(&mut self, invitation: &Invitation) -> Result<(), StoreError>;
}

/// Password reset links.
#[async_trait]
pub trait PasswordResetRepo: Send {
    /// Insert a reset.
    async fn insert(&mut self, reset: &PasswordReset) -> Result<(), StoreError>;
    /// Fetch by the hash of its link token.
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<PasswordReset>, StoreError>;
    /// Invalidate every unused reset for the user; returns how many were invalidated.
    async fn invalidate_unused_for_user(
        &mut self,
        user_id: UserId,
        now: OffsetDateTime,
    ) -> Result<u64, StoreError>;
    /// Save changes.
    async fn update(&mut self, reset: &PasswordReset) -> Result<(), StoreError>;
}

/// Sign-ins.
#[async_trait]
pub trait SessionRepo: Send {
    /// Fetch by id.
    async fn get(&mut self, id: AuthSessionId) -> Result<Option<AuthSession>, StoreError>;
    /// Insert a sign-in.
    async fn insert(&mut self, session: &AuthSession) -> Result<(), StoreError>;
    /// Fetch by the hash of its cookie token.
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<AuthSession>, StoreError>;
    /// Save changes (touch, end, scope upgrade).
    async fn update(&mut self, session: &AuthSession) -> Result<(), StoreError>;
    /// End every active sign-in of the user, optionally sparing one; returns how many ended.
    async fn end_all_for_user(
        &mut self,
        user_id: UserId,
        reason: EndReason,
        now: OffsetDateTime,
        except: Option<AuthSessionId>,
    ) -> Result<u64, StoreError>;
}

/// MFA credentials and recovery codes.
#[async_trait]
pub trait MfaRepo: Send {
    /// The user's credential, confirmed or not.
    async fn get_credential(
        &mut self,
        user_id: UserId,
    ) -> Result<Option<MfaCredential>, StoreError>;
    /// Create or replace the user's credential.
    async fn upsert_credential(&mut self, credential: &MfaCredential) -> Result<(), StoreError>;
    /// Remove the credential and all recovery codes.
    async fn delete_credential_and_codes(&mut self, user_id: UserId) -> Result<(), StoreError>;
    /// Replace all of the user's recovery codes with a new set.
    async fn replace_recovery_codes(
        &mut self,
        user_id: UserId,
        codes: &[RecoveryCode],
    ) -> Result<(), StoreError>;
    /// Find an unused-or-used recovery code by hash.
    async fn find_recovery_code(
        &mut self,
        user_id: UserId,
        hash: &TokenHash,
    ) -> Result<Option<RecoveryCode>, StoreError>;
    /// Save a recovery code (after use).
    async fn update_recovery_code(&mut self, code: &RecoveryCode) -> Result<(), StoreError>;
    /// How many recovery codes remain unused.
    async fn count_unused_recovery_codes(&mut self, user_id: UserId) -> Result<usize, StoreError>;
}

/// Sign-in guards (BR-3).
#[async_trait]
pub trait GuardRepo: Send {
    /// The guard of an account, locked for update until the transaction ends. A missing row is a
    /// fresh [`GuardState`].
    async fn get_account_guard_for_update(
        &mut self,
        user_id: UserId,
    ) -> Result<GuardState, StoreError>;
    /// Save an account's guard.
    async fn put_account_guard(
        &mut self,
        user_id: UserId,
        state: &GuardState,
    ) -> Result<(), StoreError>;
    /// The guard of an unknown email, keyed by the hash of the normalised email (BR-3.5).
    async fn get_unknown_guard_for_update(
        &mut self,
        key: &TokenHash,
    ) -> Result<GuardState, StoreError>;
    /// Save an unknown-email guard; the adapter keeps this table bounded.
    async fn put_unknown_guard(
        &mut self,
        key: &TokenHash,
        state: &GuardState,
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
}

/// Instance settings.
#[async_trait]
pub trait SettingsRepo: Send {
    /// Every stored key and value.
    async fn get_all(&mut self) -> Result<BTreeMap<String, String>, StoreError>;
    /// Store one value.
    async fn put(
        &mut self,
        key: &str,
        value: &str,
        updated_by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
}

/// Filter for reading the audit log.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Only this actor.
    pub actor_id: Option<UserId>,
    /// Only this event type.
    pub event_type: Option<AuditEventType>,
    /// Not before this time.
    pub from: Option<OffsetDateTime>,
    /// Not after this time.
    pub to: Option<OffsetDateTime>,
}

/// Position in the audit log for paging (newest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditCursor {
    /// Time of the last entry returned.
    pub occurred_at: OffsetDateTime,
    /// Id of the last entry returned.
    pub id: AuditEntryId,
}

/// One page of audit entries.
#[derive(Debug, Clone)]
pub struct AuditPage {
    /// Entries, newest first.
    pub entries: Vec<AuditEntry>,
    /// Pass back to get the next page; `None` at the end.
    pub next: Option<AuditCursor>,
}

/// The audit log. **There is no update or delete method, by design** (SECURITY-14).
#[async_trait]
pub trait AuditRepo: Send {
    /// Append an entry.
    async fn append(&mut self, entry: &AuditEntry) -> Result<(), StoreError>;
    /// Read a page, newest first.
    async fn query(
        &mut self,
        filter: &AuditFilter,
        after: Option<AuditCursor>,
        limit: usize,
    ) -> Result<AuditPage, StoreError>;
}

/// Skeleton chat.
#[async_trait]
pub trait ChatRepo: Send {
    /// Insert a session.
    async fn insert_session(&mut self, session: &ChatSession) -> Result<(), StoreError>;
    /// Fetch a session by id.
    async fn get_session(&mut self, id: ChatSessionId) -> Result<Option<ChatSession>, StoreError>;
    /// The owner's sessions, newest first.
    async fn list_sessions_for_owner(
        &mut self,
        owner: UserId,
    ) -> Result<Vec<ChatSession>, StoreError>;
    /// Insert a message.
    async fn insert_message(&mut self, message: &ChatMessage) -> Result<(), StoreError>;
    /// Fetch a message by id.
    async fn get_message(&mut self, id: ChatMessageId) -> Result<Option<ChatMessage>, StoreError>;
    /// A session's messages, oldest first (at most `limit`). Messages with the same timestamp keep
    /// insertion order, so a reply never sorts before the message it answers.
    async fn list_messages(
        &mut self,
        session: ChatSessionId,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, StoreError>;
    /// Whether an agent reply to this message already exists (idempotent replies, BR-13.4). Inserting a
    /// second reply to the same message fails with `StoreError::Conflict("chat_messages.in_reply_to")`.
    async fn has_agent_reply_to(&mut self, message_id: ChatMessageId) -> Result<bool, StoreError>;
}

/// The transactional outbox (BR-13.4).
#[async_trait]
pub trait OutboxRepo: Send {
    /// Append an event. The adapter issues a wake-up notification when the transaction commits.
    async fn append(
        &mut self,
        event: &NewOutboxEvent,
        now: OffsetDateTime,
    ) -> Result<OutboxEventId, StoreError>;
    /// Unpublished events in id order.
    async fn fetch_unpublished(&mut self, limit: usize) -> Result<Vec<OutboxEvent>, StoreError>;
    /// Mark events as published.
    async fn mark_published(
        &mut self,
        ids: &[OutboxEventId],
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
    /// Events after the given id in id order (event fan-out and replay).
    async fn fetch_after(
        &mut self,
        after: OutboxEventId,
        limit: usize,
    ) -> Result<Vec<OutboxEvent>, StoreError>;
    /// The lowest retained event id, or `None` when the outbox is empty (replay window start).
    async fn lowest_retained_id(&mut self) -> Result<Option<OutboxEventId>, StoreError>;
    /// The highest event id written so far, or `None` when nothing was ever written.
    async fn latest_id(&mut self) -> Result<Option<OutboxEventId>, StoreError>;
}

/// The job queue.
#[async_trait]
pub trait JobQueue: Send {
    /// Enqueue a job.
    async fn enqueue(&mut self, job: &Job) -> Result<(), StoreError>;
    /// Atomically lease the next due job (`SKIP LOCKED`), starting an attempt.
    async fn lease_next(
        &mut self,
        now: OffsetDateTime,
        ttl: Duration,
    ) -> Result<Option<Job>, StoreError>;
    /// Save a job after a domain transition (heartbeat, complete, fail).
    async fn save(&mut self, job: &Job) -> Result<(), StoreError>;
    /// Fetch a job by id.
    async fn get(&mut self, id: JobId) -> Result<Option<Job>, StoreError>;
    /// Apply failure handling to every job whose lease has lapsed; returns how many.
    async fn recover_lapsed_leases(&mut self, now: OffsetDateTime) -> Result<u64, StoreError>;
}
