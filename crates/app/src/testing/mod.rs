//! In-memory fakes for tests: a transactional store, a deterministic password hasher, breach list,
//! TOTP and vault, and a ready-made [`TestApp`]. Enabled by the `testing` feature.

mod config;

pub use config::{
    ConfigState, MemoryLockGuard, MemoryLocks, ProbeCalls, ScriptedProbes, StoredProvider,
    StoredSearch,
};

use crate::context::RequestContext;
use crate::deps::{App, Dependencies, Integrations};
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tandem_core::audit::AuditEntry;
use tandem_core::chat::{ChatMessage, ChatSession};
use tandem_core::email::Email;
use tandem_core::ids::{
    AuthSessionId, ChatMessageId, ChatSessionId, InvitationId, JobId, OutboxEventId, UserId,
};
use tandem_core::invitation::Invitation;
use tandem_core::job::{Job, JobStatus};
use tandem_core::mfa::{MfaCredential, RecoveryCode};
use tandem_core::outbox::{NewOutboxEvent, OutboxEvent};
use tandem_core::reset::PasswordReset;
use tandem_core::session::{AuthSession, EndReason};
use tandem_core::throttle::GuardState;
use tandem_core::token::TokenHash;
use tandem_core::user::User;
use tandem_ports::{
    AuditCursor, AuditFilter, AuditPage, AuditRepo, BreachChecker, ChatRepo, ConnectorRepo,
    DefaultsRepo, GuardRepo, HashError, InvitationRepo, JobQueue, MfaRepo, OutboxRepo,
    PasswordHasher, PasswordResetRepo, PolicyRepo, ProviderRepo, SearchRepo, SessionRepo,
    SettingsRepo, Store, StoreError, TestRunRepo, TotpService, Tx, UserRepo, Vault, VaultError,
    Verification,
};
use tandem_providers::MockProvider;
use tandem_support::{Clock, FakeClock, FakeRng, Rng, RngError, random_array};
use time::{Duration, OffsetDateTime};

/// Everything the in-memory store holds.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Users.
    pub users: Vec<User>,
    /// Invitations.
    pub invitations: Vec<Invitation>,
    /// Password resets.
    pub resets: Vec<PasswordReset>,
    /// Sign-ins.
    pub sessions: Vec<AuthSession>,
    /// MFA credentials.
    pub credentials: Vec<MfaCredential>,
    /// Recovery codes.
    pub recovery_codes: Vec<RecoveryCode>,
    /// Account guards.
    pub account_guards: HashMap<UserId, GuardState>,
    /// Unknown-email guards.
    pub unknown_guards: HashMap<String, GuardState>,
    /// Settings.
    pub settings: BTreeMap<String, String>,
    /// Audit entries.
    pub audit: Vec<AuditEntry>,
    /// Skeleton sessions.
    pub chat_sessions: Vec<ChatSession>,
    /// Skeleton messages.
    pub messages: Vec<ChatMessage>,
    /// Outbox events.
    pub outbox: Vec<OutboxEvent>,
    /// Jobs.
    pub jobs: Vec<Job>,
    /// Instance configuration (U2).
    pub config: ConfigState,
    /// When true, every audit append fails (to test fail-safe behaviour, BR-11.1).
    pub fail_audit: bool,
    next_outbox_id: i64,
}

/// An in-memory [`Store`]. Transactions work on a snapshot and replace the shared state on commit;
/// the two admin locks serialise transactions that take them.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    shared: Arc<Mutex<State>>,
    gate: Arc<tokio::sync::Mutex<()>>,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the committed state.
    pub fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.shared.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Change the committed state directly (test set-up).
    pub fn write<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.shared.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

struct MemoryTx {
    shared: Arc<Mutex<State>>,
    gate: Arc<tokio::sync::Mutex<()>>,
    work: State,
    _lock: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl MemoryTx {
    async fn take_lock(&mut self) {
        if self._lock.is_none() {
            self._lock = Some(Arc::clone(&self.gate).lock_owned().await);
            self.work = self
                .shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
        }
    }
}

#[async_trait]
impl Store for MemoryStore {
    async fn begin(&self) -> Result<Box<dyn Tx>, StoreError> {
        let work = self
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Ok(Box::new(MemoryTx {
            shared: Arc::clone(&self.shared),
            gate: Arc::clone(&self.gate),
            work,
            _lock: None,
        }))
    }
}

#[async_trait]
impl Tx for MemoryTx {
    fn users(&mut self) -> &mut dyn UserRepo {
        &mut self.work
    }
    fn invitations(&mut self) -> &mut dyn InvitationRepo {
        &mut self.work
    }
    fn resets(&mut self) -> &mut dyn PasswordResetRepo {
        &mut self.work
    }
    fn sessions(&mut self) -> &mut dyn SessionRepo {
        &mut self.work
    }
    fn mfa(&mut self) -> &mut dyn MfaRepo {
        &mut self.work
    }
    fn guards(&mut self) -> &mut dyn GuardRepo {
        &mut self.work
    }
    fn settings(&mut self) -> &mut dyn SettingsRepo {
        &mut self.work
    }
    fn audit(&mut self) -> &mut dyn AuditRepo {
        &mut self.work
    }
    fn chat(&mut self) -> &mut dyn ChatRepo {
        &mut self.work
    }
    fn outbox(&mut self) -> &mut dyn OutboxRepo {
        &mut self.work
    }
    fn jobs(&mut self) -> &mut dyn JobQueue {
        &mut self.work
    }
    fn providers(&mut self) -> &mut dyn ProviderRepo {
        &mut self.work
    }
    fn defaults(&mut self) -> &mut dyn DefaultsRepo {
        &mut self.work
    }
    fn search(&mut self) -> &mut dyn SearchRepo {
        &mut self.work
    }
    fn connectors(&mut self) -> &mut dyn ConnectorRepo {
        &mut self.work
    }
    fn policies(&mut self) -> &mut dyn PolicyRepo {
        &mut self.work
    }
    fn test_runs(&mut self) -> &mut dyn TestRunRepo {
        &mut self.work
    }

    async fn lock_setup(&mut self) -> Result<(), StoreError> {
        self.take_lock().await;
        Ok(())
    }

    async fn lock_admin_changes(&mut self) -> Result<(), StoreError> {
        self.take_lock().await;
        Ok(())
    }

    async fn commit(self: Box<Self>) -> Result<(), StoreError> {
        let this = *self;
        *this.shared.lock().unwrap_or_else(PoisonError::into_inner) = this.work;
        Ok(())
    }
}

#[async_trait]
impl UserRepo for State {
    async fn insert(&mut self, user: &User) -> Result<(), StoreError> {
        if self
            .users
            .iter()
            .any(|existing| existing.email == user.email)
        {
            return Err(StoreError::Conflict("users.email"));
        }
        self.users.push(user.clone());
        Ok(())
    }
    async fn get(&mut self, id: UserId) -> Result<Option<User>, StoreError> {
        Ok(self.users.iter().find(|user| user.id == id).cloned())
    }
    async fn find_by_email(&mut self, email: &Email) -> Result<Option<User>, StoreError> {
        Ok(self.users.iter().find(|user| &user.email == email).cloned())
    }
    async fn list(&mut self) -> Result<Vec<User>, StoreError> {
        Ok(self.users.clone())
    }
    async fn update(&mut self, user: &User) -> Result<(), StoreError> {
        let slot = self
            .users
            .iter_mut()
            .find(|existing| existing.id == user.id)
            .ok_or(StoreError::NotFound)?;
        *slot = user.clone();
        Ok(())
    }
    async fn count_all(&mut self) -> Result<u64, StoreError> {
        Ok(self.users.len() as u64)
    }
    async fn count_active_admins(&mut self) -> Result<u64, StoreError> {
        Ok(self
            .users
            .iter()
            .filter(|user| user.is_active_admin())
            .count() as u64)
    }
}

#[async_trait]
impl InvitationRepo for State {
    async fn insert(&mut self, invitation: &Invitation) -> Result<(), StoreError> {
        self.invitations.push(invitation.clone());
        Ok(())
    }
    async fn get(&mut self, id: InvitationId) -> Result<Option<Invitation>, StoreError> {
        Ok(self
            .invitations
            .iter()
            .find(|invitation| invitation.id == id)
            .cloned())
    }
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<Invitation>, StoreError> {
        Ok(self
            .invitations
            .iter()
            .find(|invitation| &invitation.token_hash == hash)
            .cloned())
    }
    async fn find_pending_by_email(
        &mut self,
        email: &Email,
        now: OffsetDateTime,
    ) -> Result<Option<Invitation>, StoreError> {
        Ok(self
            .invitations
            .iter()
            .find(|invitation| &invitation.email == email && invitation.is_usable(now))
            .cloned())
    }
    async fn list(&mut self) -> Result<Vec<Invitation>, StoreError> {
        let mut all = self.invitations.clone();
        all.sort_by_key(|invitation| std::cmp::Reverse(invitation.created_at));
        Ok(all)
    }
    async fn update(&mut self, invitation: &Invitation) -> Result<(), StoreError> {
        let slot = self
            .invitations
            .iter_mut()
            .find(|existing| existing.id == invitation.id)
            .ok_or(StoreError::NotFound)?;
        *slot = invitation.clone();
        Ok(())
    }
}

#[async_trait]
impl PasswordResetRepo for State {
    async fn insert(&mut self, reset: &PasswordReset) -> Result<(), StoreError> {
        self.resets.push(reset.clone());
        Ok(())
    }
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<PasswordReset>, StoreError> {
        Ok(self
            .resets
            .iter()
            .find(|reset| &reset.token_hash == hash)
            .cloned())
    }
    async fn invalidate_unused_for_user(
        &mut self,
        user_id: UserId,
        now: OffsetDateTime,
    ) -> Result<u64, StoreError> {
        let mut count = 0;
        for reset in self
            .resets
            .iter_mut()
            .filter(|reset| reset.user_id == user_id && reset.used_at.is_none())
        {
            reset.invalidate(now);
            count += 1;
        }
        Ok(count)
    }
    async fn update(&mut self, reset: &PasswordReset) -> Result<(), StoreError> {
        let slot = self
            .resets
            .iter_mut()
            .find(|existing| existing.id == reset.id)
            .ok_or(StoreError::NotFound)?;
        *slot = reset.clone();
        Ok(())
    }
}

#[async_trait]
impl SessionRepo for State {
    async fn insert(&mut self, session: &AuthSession) -> Result<(), StoreError> {
        self.sessions.push(session.clone());
        Ok(())
    }
    async fn get(&mut self, id: AuthSessionId) -> Result<Option<AuthSession>, StoreError> {
        Ok(self
            .sessions
            .iter()
            .find(|session| session.id == id)
            .cloned())
    }
    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<AuthSession>, StoreError> {
        Ok(self
            .sessions
            .iter()
            .find(|session| &session.token_hash == hash)
            .cloned())
    }
    async fn update(&mut self, session: &AuthSession) -> Result<(), StoreError> {
        let slot = self
            .sessions
            .iter_mut()
            .find(|existing| existing.id == session.id)
            .ok_or(StoreError::NotFound)?;
        *slot = session.clone();
        Ok(())
    }
    async fn end_all_for_user(
        &mut self,
        user_id: UserId,
        reason: EndReason,
        now: OffsetDateTime,
        except: Option<AuthSessionId>,
    ) -> Result<u64, StoreError> {
        let mut count = 0;
        for session in self
            .sessions
            .iter_mut()
            .filter(|s| s.user_id == user_id && s.ended_reason.is_none())
        {
            if Some(session.id) != except {
                session.end(reason, now);
                count += 1;
            }
        }
        Ok(count)
    }
}

#[async_trait]
impl MfaRepo for State {
    async fn get_credential(
        &mut self,
        user_id: UserId,
    ) -> Result<Option<MfaCredential>, StoreError> {
        Ok(self
            .credentials
            .iter()
            .find(|credential| credential.user_id == user_id)
            .cloned())
    }
    async fn upsert_credential(&mut self, credential: &MfaCredential) -> Result<(), StoreError> {
        match self
            .credentials
            .iter_mut()
            .find(|existing| existing.user_id == credential.user_id)
        {
            Some(slot) => *slot = credential.clone(),
            None => self.credentials.push(credential.clone()),
        }
        Ok(())
    }
    async fn delete_credential_and_codes(&mut self, user_id: UserId) -> Result<(), StoreError> {
        self.credentials
            .retain(|credential| credential.user_id != user_id);
        self.recovery_codes.retain(|code| code.user_id != user_id);
        Ok(())
    }
    async fn replace_recovery_codes(
        &mut self,
        user_id: UserId,
        codes: &[RecoveryCode],
    ) -> Result<(), StoreError> {
        self.recovery_codes.retain(|code| code.user_id != user_id);
        self.recovery_codes.extend_from_slice(codes);
        Ok(())
    }
    async fn find_recovery_code(
        &mut self,
        user_id: UserId,
        hash: &TokenHash,
    ) -> Result<Option<RecoveryCode>, StoreError> {
        Ok(self
            .recovery_codes
            .iter()
            .find(|code| code.user_id == user_id && &code.code_hash == hash)
            .cloned())
    }
    async fn update_recovery_code(&mut self, code: &RecoveryCode) -> Result<(), StoreError> {
        let slot = self
            .recovery_codes
            .iter_mut()
            .find(|existing| existing.id == code.id)
            .ok_or(StoreError::NotFound)?;
        *slot = code.clone();
        Ok(())
    }
    async fn count_unused_recovery_codes(&mut self, user_id: UserId) -> Result<usize, StoreError> {
        Ok(self
            .recovery_codes
            .iter()
            .filter(|code| code.user_id == user_id && code.used_at.is_none())
            .count())
    }
}

#[async_trait]
impl GuardRepo for State {
    async fn get_account_guard_for_update(
        &mut self,
        user_id: UserId,
    ) -> Result<GuardState, StoreError> {
        Ok(self
            .account_guards
            .get(&user_id)
            .cloned()
            .unwrap_or_default())
    }
    async fn put_account_guard(
        &mut self,
        user_id: UserId,
        state: &GuardState,
    ) -> Result<(), StoreError> {
        self.account_guards.insert(user_id, state.clone());
        Ok(())
    }
    async fn get_unknown_guard_for_update(
        &mut self,
        key: &TokenHash,
    ) -> Result<GuardState, StoreError> {
        Ok(self
            .unknown_guards
            .get(key.as_str())
            .cloned()
            .unwrap_or_default())
    }
    async fn put_unknown_guard(
        &mut self,
        key: &TokenHash,
        state: &GuardState,
        _now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        self.unknown_guards
            .insert(key.as_str().to_owned(), state.clone());
        Ok(())
    }
}

#[async_trait]
impl SettingsRepo for State {
    async fn get_all(&mut self) -> Result<BTreeMap<String, String>, StoreError> {
        Ok(self.settings.clone())
    }
    async fn put(
        &mut self,
        key: &str,
        value: &str,
        _updated_by: UserId,
        _now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        self.settings.insert(key.to_owned(), value.to_owned());
        Ok(())
    }
}

#[async_trait]
impl AuditRepo for State {
    async fn append(&mut self, entry: &AuditEntry) -> Result<(), StoreError> {
        if self.fail_audit {
            return Err(StoreError::Other("injected audit failure".into()));
        }
        self.audit.push(entry.clone());
        Ok(())
    }
    async fn query(
        &mut self,
        filter: &AuditFilter,
        after: Option<AuditCursor>,
        limit: usize,
    ) -> Result<AuditPage, StoreError> {
        let mut matching: Vec<&AuditEntry> = self
            .audit
            .iter()
            .filter(|entry| {
                filter
                    .actor_id
                    .is_none_or(|id| entry.event.actor_id == Some(id))
            })
            .filter(|entry| {
                filter
                    .event_type
                    .is_none_or(|ty| entry.event.event_type == ty)
            })
            .filter(|entry| {
                filter
                    .from
                    .is_none_or(|from| entry.event.occurred_at >= from)
            })
            .filter(|entry| filter.to.is_none_or(|to| entry.event.occurred_at <= to))
            .collect();
        matching.sort_by_key(|entry| std::cmp::Reverse((entry.event.occurred_at, entry.id)));
        let page: Vec<AuditEntry> = matching
            .into_iter()
            .filter(|entry| {
                after.is_none_or(|c| (entry.event.occurred_at, entry.id) < (c.occurred_at, c.id))
            })
            .take(limit + 1)
            .cloned()
            .collect();
        let has_more = page.len() > limit;
        let entries: Vec<AuditEntry> = page.into_iter().take(limit).collect();
        let next = if has_more {
            entries.last().map(|entry| AuditCursor {
                occurred_at: entry.event.occurred_at,
                id: entry.id,
            })
        } else {
            None
        };
        Ok(AuditPage { entries, next })
    }
}

#[async_trait]
impl ChatRepo for State {
    async fn insert_session(&mut self, session: &ChatSession) -> Result<(), StoreError> {
        self.chat_sessions.push(session.clone());
        Ok(())
    }
    async fn get_session(&mut self, id: ChatSessionId) -> Result<Option<ChatSession>, StoreError> {
        Ok(self
            .chat_sessions
            .iter()
            .find(|session| session.id == id)
            .cloned())
    }
    async fn list_sessions_for_owner(
        &mut self,
        owner: UserId,
    ) -> Result<Vec<ChatSession>, StoreError> {
        let mut own: Vec<ChatSession> = self
            .chat_sessions
            .iter()
            .filter(|s| s.owner_id == owner)
            .cloned()
            .collect();
        own.sort_by_key(|session| std::cmp::Reverse(session.created_at));
        Ok(own)
    }
    async fn insert_message(&mut self, message: &ChatMessage) -> Result<(), StoreError> {
        if let Some(parent) = message.in_reply_to
            && self
                .messages
                .iter()
                .any(|existing| existing.in_reply_to == Some(parent))
        {
            return Err(StoreError::Conflict("chat_messages.in_reply_to"));
        }
        self.messages.push(message.clone());
        Ok(())
    }
    async fn get_message(&mut self, id: ChatMessageId) -> Result<Option<ChatMessage>, StoreError> {
        Ok(self
            .messages
            .iter()
            .find(|message| message.id == id)
            .cloned())
    }
    async fn list_messages(
        &mut self,
        session: ChatSessionId,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, StoreError> {
        let mut all: Vec<ChatMessage> = self
            .messages
            .iter()
            .filter(|m| m.chat_session_id == session)
            .cloned()
            .collect();
        all.truncate(limit);
        Ok(all)
    }
    async fn has_agent_reply_to(&mut self, message_id: ChatMessageId) -> Result<bool, StoreError> {
        Ok(self
            .messages
            .iter()
            .any(|message| message.in_reply_to == Some(message_id)))
    }
}

#[async_trait]
impl OutboxRepo for State {
    async fn append(
        &mut self,
        event: &NewOutboxEvent,
        now: OffsetDateTime,
    ) -> Result<OutboxEventId, StoreError> {
        self.next_outbox_id += 1;
        let id = OutboxEventId(self.next_outbox_id);
        self.outbox.push(OutboxEvent {
            id,
            created_at: now,
            payload: event.payload.clone(),
            trace_context: event.trace_context.clone(),
            published_at: None,
        });
        Ok(id)
    }
    async fn fetch_unpublished(&mut self, limit: usize) -> Result<Vec<OutboxEvent>, StoreError> {
        Ok(self
            .outbox
            .iter()
            .filter(|event| event.published_at.is_none())
            .take(limit)
            .cloned()
            .collect())
    }
    async fn mark_published(
        &mut self,
        ids: &[OutboxEventId],
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        for event in self
            .outbox
            .iter_mut()
            .filter(|event| ids.contains(&event.id))
        {
            event.published_at = Some(now);
        }
        Ok(())
    }
    async fn fetch_after(
        &mut self,
        after: OutboxEventId,
        limit: usize,
    ) -> Result<Vec<OutboxEvent>, StoreError> {
        Ok(self
            .outbox
            .iter()
            .filter(|event| event.id > after)
            .take(limit)
            .cloned()
            .collect())
    }
    async fn lowest_retained_id(&mut self) -> Result<Option<OutboxEventId>, StoreError> {
        Ok(self.outbox.first().map(|event| event.id))
    }
    async fn latest_id(&mut self) -> Result<Option<OutboxEventId>, StoreError> {
        Ok(self.outbox.last().map(|event| event.id))
    }
}

#[async_trait]
impl JobQueue for State {
    async fn enqueue(&mut self, job: &Job) -> Result<(), StoreError> {
        self.jobs.push(job.clone());
        Ok(())
    }
    async fn lease_next(
        &mut self,
        now: OffsetDateTime,
        ttl: Duration,
    ) -> Result<Option<Job>, StoreError> {
        let mut due: Vec<usize> = (0..self.jobs.len())
            .filter(|&i| self.jobs[i].status == JobStatus::Queued && self.jobs[i].run_at <= now)
            .collect();
        due.sort_by_key(|&i| (self.jobs[i].run_at, self.jobs[i].id));
        let Some(&index) = due.first() else {
            return Ok(None);
        };
        let job = &mut self.jobs[index];
        job.lease(now, ttl)
            .map_err(|error| StoreError::Other(error.to_string().into()))?;
        Ok(Some(job.clone()))
    }
    async fn save(&mut self, job: &Job) -> Result<(), StoreError> {
        let slot = self
            .jobs
            .iter_mut()
            .find(|existing| existing.id == job.id)
            .ok_or(StoreError::NotFound)?;
        *slot = job.clone();
        Ok(())
    }
    async fn get(&mut self, id: JobId) -> Result<Option<Job>, StoreError> {
        Ok(self.jobs.iter().find(|job| job.id == id).cloned())
    }
    async fn recover_lapsed_leases(&mut self, now: OffsetDateTime) -> Result<u64, StoreError> {
        let mut count = 0;
        for job in &mut self.jobs {
            if job.recover_if_lease_lapsed(now).is_some() {
                count += 1;
            }
        }
        Ok(count)
    }
}

/// A deterministic hasher: `fake$v2$<password>` is the current format; `fake$v1$<password>` is an
/// outdated one that verifies but asks for a rehash.
#[derive(Debug, Default)]
pub struct FakeHasher {
    /// When true, every operation fails with [`HashError::Busy`].
    pub busy: AtomicBool,
    /// How many times `dummy_verify` ran (unknown-account path, BR-3.5).
    pub dummy_calls: AtomicUsize,
    /// How many real verifications ran.
    pub verify_calls: AtomicUsize,
}

#[async_trait]
impl PasswordHasher for FakeHasher {
    async fn hash(&self, password: &str) -> Result<String, HashError> {
        if self.busy.load(Ordering::SeqCst) {
            return Err(HashError::Busy);
        }
        Ok(format!("fake$v2${password}"))
    }
    async fn verify(&self, password: &str, stored: &str) -> Result<Verification, HashError> {
        if self.busy.load(Ordering::SeqCst) {
            return Err(HashError::Busy);
        }
        self.verify_calls.fetch_add(1, Ordering::SeqCst);
        let current = stored.strip_prefix("fake$v2$");
        let outdated = stored.strip_prefix("fake$v1$");
        Ok(Verification {
            matches: current.or(outdated) == Some(password),
            needs_rehash: outdated.is_some(),
        })
    }
    async fn dummy_verify(&self) -> Result<(), HashError> {
        if self.busy.load(Ordering::SeqCst) {
            return Err(HashError::Busy);
        }
        self.dummy_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A breach list held in memory.
#[derive(Debug, Default)]
pub struct FakeBreachChecker {
    breached: Mutex<HashSet<String>>,
    /// Whether the last check asked for the online check.
    pub last_used_online: AtomicBool,
}

impl FakeBreachChecker {
    /// Mark a password as breached.
    pub fn add(&self, password: &str) {
        self.breached
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(password.to_owned());
    }
}

#[async_trait]
impl BreachChecker for FakeBreachChecker {
    async fn is_breached(&self, password: &str, use_online: bool) -> bool {
        self.last_used_online.store(use_online, Ordering::SeqCst);
        self.breached
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(password)
    }
}

/// A deterministic TOTP: the code for a step is derived from the secret and the step number.
#[derive(Debug)]
pub struct FakeTotp {
    rng: Arc<dyn Rng>,
}

impl FakeTotp {
    /// A fake TOTP drawing secrets from the given random source.
    pub fn new(rng: Arc<dyn Rng>) -> Self {
        Self { rng }
    }

    /// The code that is valid at `at` for the secret.
    pub fn code_at(&self, secret: &[u8], at: OffsetDateTime) -> String {
        Self::code_for(secret, Self::step_at(at))
    }

    fn step_at(at: OffsetDateTime) -> u64 {
        u64::try_from(at.unix_timestamp()).unwrap_or(0) / 30
    }

    fn code_for(secret: &[u8], step: u64) -> String {
        let sum: u64 = secret.iter().map(|byte| u64::from(*byte)).sum();
        format!("{:06}", (sum * 7919 + step * 104_729) % 1_000_000)
    }
}

impl TotpService for FakeTotp {
    fn generate_secret(&self) -> Result<Vec<u8>, RngError> {
        Ok(random_array::<20>(&*self.rng)?.to_vec())
    }
    fn secret_base32(&self, secret: &[u8]) -> String {
        secret.iter().map(|byte| format!("{byte:02X}")).collect()
    }
    fn otpauth_uri(&self, secret: &[u8], account: &str, issuer: &str) -> String {
        format!(
            "otpauth://totp/{issuer}:{account}?secret={}&issuer={issuer}",
            self.secret_base32(secret)
        )
    }
    fn verify(&self, secret: &[u8], code: &str, at: OffsetDateTime) -> Option<u64> {
        let now = Self::step_at(at);
        (now.saturating_sub(1)..=now + 1).find(|step| Self::code_for(secret, *step) == code)
    }
}

/// A vault that binds a sealed value to its purpose and row, like the real one.
#[derive(Debug, Default)]
pub struct FakeVault;

impl Vault for FakeVault {
    fn seal(&self, purpose: &str, row_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let mut sealed = format!("sealed:{purpose}:{row_id}:").into_bytes();
        sealed.extend_from_slice(plaintext);
        Ok(sealed)
    }
    fn open(&self, purpose: &str, row_id: &str, sealed: &[u8]) -> Result<Vec<u8>, VaultError> {
        let prefix = format!("sealed:{purpose}:{row_id}:").into_bytes();
        sealed
            .strip_prefix(prefix.as_slice())
            .map(<[u8]>::to_vec)
            .ok_or(VaultError::Open)
    }
}

/// An application wired entirely to fakes, with handles to control them.
pub struct TestApp {
    /// The application under test.
    pub app: App,
    /// The in-memory store.
    pub store: MemoryStore,
    /// The controllable clock.
    pub clock: FakeClock,
    /// The password hasher fake.
    pub hasher: Arc<FakeHasher>,
    /// The breach list fake.
    pub breach: Arc<FakeBreachChecker>,
    /// The TOTP fake.
    pub totp: Arc<FakeTotp>,
    /// The per-target lock shared by every worker in the test.
    pub locks: MemoryLocks,
    /// The scripted probes; set what they answer and read what they were given.
    pub probes: ScriptedProbes,
}

/// A password that satisfies the policy.
pub const GOOD_PASSWORD: &str = "Correct-Horse-7";

impl TestApp {
    /// A fresh application: empty store, clock at a fixed instant, mock provider.
    pub fn new() -> Self {
        Self::with_provider(MockProvider::new())
    }

    /// Like [`Self::new`] with a chosen mock provider.
    pub fn with_provider(provider: MockProvider) -> Self {
        let store = MemoryStore::new();
        let clock = FakeClock::at_unix(1_800_000_000);
        let rng: Arc<dyn Rng> = Arc::new(FakeRng::new(2026));
        let hasher = Arc::new(FakeHasher::default());
        let breach = Arc::new(FakeBreachChecker::default());
        let totp = Arc::new(FakeTotp::new(Arc::clone(&rng)));
        let locks = MemoryLocks::default();
        let probes = ScriptedProbes::default();
        let app = App::new(Dependencies {
            store: Arc::new(store.clone()),
            clock: Arc::new(clock.clone()),
            rng,
            hasher: hasher.clone(),
            breach: breach.clone(),
            totp: totp.clone(),
            vault: Arc::new(FakeVault),
            provider: Arc::new(provider),
            issuer: "Tandem".to_owned(),
            integrations: Integrations {
                locks: Arc::new(locks.clone()),
                provider_probe: Arc::new(probes.clone()),
                search_probe: Arc::new(probes.clone()),
                connector_probe: Arc::new(probes.clone()),
                runtime_probe: Arc::new(probes.clone()),
            },
        });
        Self {
            app,
            store,
            clock,
            hasher,
            breach,
            totp,
            locks,
            probes,
        }
    }

    /// An anonymous request context.
    pub fn anonymous(&self) -> RequestContext {
        RequestContext::anonymous(Some("req-test".to_owned()), Some("203.0.113.7".to_owned()))
    }

    /// The context of a signed-in user, resolved from a session token like the API does.
    ///
    /// # Panics
    /// If the token is not valid (a test bug).
    #[allow(clippy::expect_used)]
    pub async fn context_for(&self, token: &str) -> RequestContext {
        let authenticated = self
            .app
            .resolve_session(token)
            .await
            .expect("valid session");
        RequestContext::authenticated(
            authenticated.actor,
            Some("req-test".to_owned()),
            Some("203.0.113.7".to_owned()),
        )
    }

    /// Run first-run setup and return the admin's (enrolment-only) session token.
    ///
    /// # Panics
    /// If setup fails (a test bug).
    #[allow(clippy::expect_used)]
    pub async fn setup_admin(&self, email: &str) -> String {
        let input = crate::SetupInput {
            display_name: "Ada Admin".to_owned(),
            email: email.to_owned(),
            password: GOOD_PASSWORD.to_owned(),
        };
        self.app
            .first_run_setup(&self.anonymous(), input)
            .await
            .expect("setup")
            .token
    }

    /// Enrol the context's user in MFA and return their recovery codes.
    ///
    /// # Panics
    /// If any step fails (a test bug).
    #[allow(clippy::expect_used)]
    pub async fn enrol_mfa(&self, ctx: &RequestContext) -> Vec<String> {
        self.app.mfa_start_enrolment(ctx).await.expect("start");
        let user_id = ctx.actor().expect("actor").user_id;
        let sealed = self.store.read(|s| {
            s.credentials
                .iter()
                .find(|c| c.user_id == user_id)
                .map(|c| c.secret_sealed.clone())
        });
        let secret = FakeVault
            .open(
                "mfa_secret",
                &user_id.to_string(),
                &sealed.expect("credential"),
            )
            .expect("open");
        let code = self.totp.code_at(&secret, self.clock.now());
        self.app
            .mfa_confirm_enrolment(ctx, &code)
            .await
            .expect("confirm")
    }

    /// The current valid TOTP code for a user's confirmed credential.
    ///
    /// # Panics
    /// If the user has no credential (a test bug).
    #[allow(clippy::expect_used)]
    pub fn current_code(&self, user_id: UserId) -> String {
        let sealed = self
            .store
            .read(|s| {
                s.credentials
                    .iter()
                    .find(|c| c.user_id == user_id)
                    .map(|c| c.secret_sealed.clone())
            })
            .expect("credential");
        let secret = FakeVault
            .open("mfa_secret", &user_id.to_string(), &sealed)
            .expect("open");
        self.totp.code_at(&secret, self.clock.now())
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TestApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestApp").finish_non_exhaustive()
    }
}
