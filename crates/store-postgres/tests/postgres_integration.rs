//! Integration tests against a real PostgreSQL 18 with TLS. They need Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestDb;
use sqlx::Row;
use sqlx::postgres::{PgConnectOptions, PgListener, PgSslMode};
use std::sync::Arc;
use std::time::Duration;
use tandem_core::audit::{AuditEntry, AuditEvent, AuditEventType, AuditOutcome};
use tandem_core::chat::{Author, ChatMessage, ChatSession};
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::ids::{
    AuditEntryId, AuthSessionId, ChatMessageId, ChatSessionId, InvitationId, JobId,
    PasswordResetId, RecoveryCodeId, UserId,
};
use tandem_core::invitation::{Invitation, InvitationStatus};
use tandem_core::job::{Job, JobKind, JobStatus};
use tandem_core::mfa::{MfaCredential, RecoveryCode};
use tandem_core::outbox::{EventPayload, NewOutboxEvent};
use tandem_core::reset::PasswordReset;
use tandem_core::role::{Role, UserStatus};
use tandem_core::session::{AuthSession, EndReason, Scope};
use tandem_core::throttle::GuardState;
use tandem_core::token::{hash_recovery_code, hash_token};
use tandem_core::user::{CreatedVia, User};
use tandem_ports::{AuditCursor, AuditFilter, Store, StoreError};
use tandem_store_postgres::{
    PoolConfig, RelayLease, SchemaError, applied_schema_version, check_schema, connect_pool,
    expected_schema_version, migrate_down_one, migrate_up,
};
use tandem_support::{Clock, FakeClock, FakeRng};
use time::{Duration as TimeDuration, OffsetDateTime};

/// Builds domain objects with deterministic ids and a fixed clock.
struct Fx {
    clock: FakeClock,
    rng: FakeRng,
}

impl Fx {
    fn new() -> Self {
        Self {
            clock: FakeClock::at_unix(1_767_225_600),
            rng: FakeRng::new(7),
        } // 2026-01-01
    }

    fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    fn user(&self, email: &str, role: Role) -> User {
        User {
            id: UserId::generate(&self.clock, &self.rng).unwrap(),
            email: Email::parse(email).unwrap(),
            display_name: DisplayName::parse("Test User").unwrap(),
            role,
            status: UserStatus::Active,
            password_hash: "$argon2id$stub".to_owned(),
            password_changed_at: self.now(),
            created_at: self.now(),
            created_via: CreatedVia::Invitation,
            deactivated_at: None,
            deactivated_by: None,
            last_sign_in_at: None,
        }
    }

    fn session(&self, user: &User, token: &str) -> AuthSession {
        AuthSession {
            id: AuthSessionId::generate(&self.clock, &self.rng).unwrap(),
            user_id: user.id,
            token_hash: hash_token(token),
            created_at: self.now(),
            last_seen_at: self.now(),
            idle_timeout: TimeDuration::hours(12),
            absolute_duration: TimeDuration::days(14),
            scope: Scope::Full,
            ended_at: None,
            ended_reason: None,
            source_summary: Some("203.0.113.7".to_owned()),
        }
    }

    fn chat_session(&self, owner: &User) -> ChatSession {
        ChatSession {
            id: ChatSessionId::generate(&self.clock, &self.rng).unwrap(),
            title: "Planning".to_owned(),
            owner_id: owner.id,
            created_at: self.now(),
        }
    }

    fn message(
        &self,
        session: &ChatSession,
        author: Author,
        body: &str,
        reply_to: Option<ChatMessageId>,
    ) -> ChatMessage {
        ChatMessage {
            id: ChatMessageId::generate(&self.clock, &self.rng).unwrap(),
            chat_session_id: session.id,
            author,
            body: body.to_owned(),
            created_at: self.now(),
            in_reply_to: reply_to,
        }
    }

    fn job(&self, message: ChatMessageId) -> Job {
        Job::new(
            JobId::generate(&self.clock, &self.rng).unwrap(),
            JobKind::MockAgentReply {
                message_id: message,
            },
            self.now(),
            Some("trace-1".to_owned()),
        )
    }

    fn audit(&self, event_type: AuditEventType, actor: Option<&User>) -> AuditEntry {
        let mut event = AuditEvent::new(event_type, AuditOutcome::Success, self.now())
            .detail("note", "grüße ✓");
        if let Some(user) = actor {
            event = event.actor(user.id, user.display_name.as_str());
        }
        AuditEntry {
            id: AuditEntryId::generate(&self.clock, &self.rng).unwrap(),
            event,
        }
    }
}

async fn insert_user(db: &TestDb, user: &User) {
    let mut tx = db.store.begin().await.unwrap();
    tx.users().insert(user).await.unwrap();
    tx.commit().await.unwrap();
}

// ---- transport and privileges -------------------------------------------------------------

#[tokio::test]
async fn se_1_plaintext_is_refused_a_wrong_ca_is_rejected_and_verified_tls_is_in_use() {
    let db = TestDb::new().await;
    let plaintext = PgConnectOptions::new()
        .host("127.0.0.1")
        .port(db.port)
        .username("tandem_app")
        .password(&db.app_password)
        .database(&db.name)
        .ssl_mode(PgSslMode::Disable);
    let refused = sqlx::ConnectOptions::connect(&plaintext).await;
    assert!(
        refused.is_err(),
        "the server must refuse a connection without encryption"
    );

    let other_ca = tempfile::tempdir().unwrap();
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let wrong = params.self_signed(&key).unwrap();
    let wrong_file = other_ca.path().join("other-ca.crt");
    std::fs::write(&wrong_file, wrong.pem()).unwrap();
    let mut config: PoolConfig = db.app_config();
    config.ca_file = wrong_file;
    assert!(
        connect_pool(&config).await.is_err(),
        "a certificate from an unknown CA must be rejected"
    );

    let row = sqlx::query("SELECT ssl, version FROM pg_stat_ssl WHERE pid = pg_backend_pid()")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert!(row.get::<bool, _>("ssl"));
    assert!(matches!(
        row.get::<String, _>("version").as_str(),
        "TLSv1.2" | "TLSv1.3"
    ));
}

#[tokio::test]
async fn se_15_the_app_role_can_append_and_read_audit_entries_but_never_change_or_delete_them() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let mut tx = db.store.begin().await.unwrap();
    tx.audit()
        .append(&fx.audit(AuditEventType::AuthSetup, None))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    for statement in [
        "UPDATE audit_entries SET outcome = 'failure'",
        "DELETE FROM audit_entries",
        "TRUNCATE audit_entries",
    ] {
        let error = sqlx::query(statement).execute(&db.app).await.unwrap_err();
        assert!(
            error.to_string().contains("permission denied"),
            "{statement}: {error}"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn se_7_the_app_role_cannot_change_the_schema_or_delete_users_but_the_migration_role_can_change_the_schema()
 {
    let db = TestDb::new().await;
    for statement in [
        "CREATE TABLE tandem.x (i int)",
        "DROP TABLE tandem.users",
        "DELETE FROM tandem.users",
        "ALTER TABLE tandem.users ADD COLUMN x int",
    ] {
        let error = sqlx::query(statement).execute(&db.app).await.unwrap_err();
        assert!(
            error.to_string().contains("permission denied")
                || error.to_string().contains("must be owner"),
            "{statement}: {error}"
        );
    }
    sqlx::query("CREATE TABLE tandem.scratch (i int)")
        .execute(&db.migrate)
        .await
        .unwrap();
    sqlx::query("DROP TABLE tandem.scratch")
        .execute(&db.migrate)
        .await
        .unwrap();
    let is_super: bool =
        sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
            .fetch_one(&db.app)
            .await
            .unwrap();
    assert!(!is_super);
}

// ---- migrations and the schema guard ------------------------------------------------------

async fn table_names(db: &TestDb) -> Vec<String> {
    sqlx::query_scalar("SELECT table_name::text FROM information_schema.tables WHERE table_schema = 'tandem' ORDER BY 1")
        .fetch_all(&db.superuser)
        .await
        .unwrap()
}

#[tokio::test]
async fn av_4_migrations_apply_revert_one_by_one_and_apply_again() {
    let db = TestDb::empty().await;
    migrate_up(&db.migrate).await.unwrap();
    assert_eq!(
        applied_schema_version(&db.migrate).await.unwrap(),
        expected_schema_version()
    );
    assert_eq!(
        table_names(&db).await.len(),
        23,
        "22 tables plus the migration ledger"
    );

    let mut reverted = Vec::new();
    while let Some(version) = migrate_down_one(&db.migrate).await.unwrap() {
        reverted.push(version);
    }
    assert_eq!(reverted, vec![5, 4, 3, 2, 1]);
    assert_eq!(table_names(&db).await, vec!["_sqlx_migrations".to_owned()]);

    migrate_up(&db.migrate).await.unwrap();
    assert_eq!(table_names(&db).await.len(), 23);
}

#[tokio::test]
async fn av_3_the_schema_guard_refuses_an_older_or_a_newer_schema() {
    let db = TestDb::empty().await;
    assert!(
        matches!(check_schema(&db.migrate).await, Err(SchemaError::Read(_))),
        "no ledger yet"
    );
    migrate_up(&db.migrate).await.unwrap();
    check_schema(&db.app)
        .await
        .expect("the app role may read the ledger");
    migrate_down_one(&db.migrate).await.unwrap();
    assert!(
        check_schema(&db.app).await.is_err(),
        "the app role fails closed on an older schema"
    );
    let older = check_schema(&db.migrate).await.unwrap_err();
    assert!(
        matches!(
            older,
            SchemaError::Mismatch {
                expected: 5,
                actual: 4
            }
        ),
        "{older}"
    );
    migrate_up(&db.migrate).await.unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (99, 'future', true, '\\x00', 0)")
        .execute(&db.migrate)
        .await
        .unwrap();
    let newer = check_schema(&db.migrate).await.unwrap_err();
    assert!(
        matches!(
            newer,
            SchemaError::Mismatch {
                expected: 5,
                actual: 99
            }
        ),
        "{newer}"
    );
}

// ---- identity repositories ----------------------------------------------------------------

#[tokio::test]
async fn users_round_trip_update_count_and_enforce_a_unique_email() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let mut admin = fx.user("ada@example.com", Role::Admin);
    let member = fx.user("mia@example.com", Role::Member);
    insert_user(&db, &admin).await;
    insert_user(&db, &member).await;

    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(tx.users().get(admin.id).await.unwrap().unwrap(), admin);
    assert_eq!(
        tx.users()
            .find_by_email(&member.email)
            .await
            .unwrap()
            .unwrap()
            .id,
        member.id
    );
    assert_eq!(tx.users().list().await.unwrap().len(), 2);
    assert_eq!(
        (
            tx.users().count_all().await.unwrap(),
            tx.users().count_active_admins().await.unwrap()
        ),
        (2, 1)
    );
    let duplicate = fx.user("ADA@example.com", Role::Member);
    assert!(matches!(
        tx.users().insert(&duplicate).await,
        Err(StoreError::Conflict("users.email"))
    ));
    drop(tx);

    admin.deactivate(member.id, fx.now()).unwrap();
    let mut tx = db.store.begin().await.unwrap();
    tx.users().update(&admin).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.store.begin().await.unwrap();
    let stored = tx.users().get(admin.id).await.unwrap().unwrap();
    assert_eq!(stored.status, UserStatus::Deactivated);
    assert_eq!(stored.deactivated_by, Some(member.id));
    assert_eq!(tx.users().count_active_admins().await.unwrap(), 0);
    assert!(matches!(
        tx.users()
            .update(&fx.user("ghost@example.com", Role::Member))
            .await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn a_dropped_transaction_rolls_back() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let mut tx = db.store.begin().await.unwrap();
    tx.users()
        .insert(&fx.user("temp@example.com", Role::Member))
        .await
        .unwrap();
    drop(tx);
    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(tx.users().count_all().await.unwrap(), 0);
}

#[tokio::test]
async fn invitations_and_resets_round_trip_and_are_found_by_token_hash() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let admin = fx.user("ada@example.com", Role::Admin);
    insert_user(&db, &admin).await;

    let mut invitation = Invitation::new(
        InvitationId::generate(&fx.clock, &fx.rng).unwrap(),
        Email::parse("new@example.com").unwrap(),
        Role::Member,
        hash_token("invite-token"),
        admin.id,
        fx.now(),
    );
    let mut reset = PasswordReset::new(
        PasswordResetId::generate(&fx.clock, &fx.rng).unwrap(),
        admin.id,
        hash_token("reset-token"),
        admin.id,
        true,
        fx.now(),
    );
    let mut tx = db.store.begin().await.unwrap();
    tx.invitations().insert(&invitation).await.unwrap();
    tx.resets().insert(&reset).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(
        tx.invitations()
            .find_by_token_hash(&hash_token("invite-token"))
            .await
            .unwrap()
            .unwrap(),
        invitation
    );
    assert!(
        tx.invitations()
            .find_by_token_hash(&hash_token("nope"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        tx.invitations()
            .get(invitation.id)
            .await
            .unwrap()
            .unwrap()
            .email
            .as_str(),
        "new@example.com"
    );
    let pending = tx
        .invitations()
        .find_pending_by_email(&invitation.email, fx.now())
        .await
        .unwrap();
    assert_eq!(pending.unwrap().id, invitation.id);
    let after_expiry = fx.now() + TimeDuration::days(8);
    assert!(
        tx.invitations()
            .find_pending_by_email(&invitation.email, after_expiry)
            .await
            .unwrap()
            .is_none()
    );
    invitation.accept(admin.id, fx.now()).unwrap();
    tx.invitations().update(&invitation).await.unwrap();
    assert!(
        tx.invitations()
            .find_pending_by_email(&invitation.email, fx.now())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        tx.invitations().list().await.unwrap()[0].status,
        InvitationStatus::Accepted
    );

    assert_eq!(
        tx.resets()
            .find_by_token_hash(&hash_token("reset-token"))
            .await
            .unwrap()
            .unwrap(),
        reset
    );
    assert_eq!(
        tx.resets()
            .invalidate_unused_for_user(admin.id, fx.now())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        tx.resets()
            .invalidate_unused_for_user(admin.id, fx.now())
            .await
            .unwrap(),
        0
    );
    reset.invalidate(fx.now());
    assert_eq!(
        tx.resets()
            .find_by_token_hash(&hash_token("reset-token"))
            .await
            .unwrap()
            .unwrap()
            .used_at,
        reset.used_at
    );
}

#[tokio::test]
async fn sessions_round_trip_and_ending_all_can_spare_one() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = fx.user("mia@example.com", Role::Member);
    insert_user(&db, &user).await;
    let (first, second) = (fx.session(&user, "token-1"), fx.session(&user, "token-2"));
    let mut tx = db.store.begin().await.unwrap();
    tx.sessions().insert(&first).await.unwrap();
    tx.sessions().insert(&second).await.unwrap();
    assert_eq!(
        tx.sessions()
            .find_by_token_hash(&hash_token("token-1"))
            .await
            .unwrap()
            .unwrap(),
        first
    );
    assert_eq!(
        tx.sessions().get(second.id).await.unwrap().unwrap().id,
        second.id
    );
    let mut touched = first.clone();
    touched.touch(fx.now() + TimeDuration::minutes(5));
    touched.scope = Scope::MfaEnrolmentOnly;
    tx.sessions().update(&touched).await.unwrap();
    assert_eq!(
        tx.sessions().get(first.id).await.unwrap().unwrap().scope,
        Scope::MfaEnrolmentOnly
    );
    let ended = tx
        .sessions()
        .end_all_for_user(
            user.id,
            EndReason::PasswordChanged,
            fx.now(),
            Some(second.id),
        )
        .await
        .unwrap();
    assert_eq!(ended, 1);
    assert_eq!(
        tx.sessions()
            .get(first.id)
            .await
            .unwrap()
            .unwrap()
            .ended_reason,
        Some(EndReason::PasswordChanged)
    );
    assert!(
        tx.sessions()
            .get(second.id)
            .await
            .unwrap()
            .unwrap()
            .ended_reason
            .is_none()
    );
    assert_eq!(
        tx.sessions()
            .end_all_for_user(user.id, EndReason::Deactivated, fx.now(), None)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn mfa_credentials_and_recovery_codes_round_trip() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = fx.user("mia@example.com", Role::Member);
    insert_user(&db, &user).await;
    let mut tx = db.store.begin().await.unwrap();
    assert!(tx.mfa().get_credential(user.id).await.unwrap().is_none());
    let mut credential = MfaCredential::new(user.id, vec![1, 2, 3, 4]);
    tx.mfa().upsert_credential(&credential).await.unwrap();
    credential.accept_step(58_000_000).unwrap();
    credential.confirm(fx.now()).unwrap();
    tx.mfa().upsert_credential(&credential).await.unwrap();
    let stored = tx.mfa().get_credential(user.id).await.unwrap().unwrap();
    assert_eq!(stored, credential);
    assert_eq!(stored.last_used_step, Some(58_000_000));

    let codes: Vec<RecoveryCode> = (0..10)
        .map(|i| RecoveryCode {
            id: RecoveryCodeId::generate(&fx.clock, &fx.rng).unwrap(),
            user_id: user.id,
            code_hash: hash_recovery_code(&format!("code-{i}")),
            used_at: None,
        })
        .collect();
    tx.mfa()
        .replace_recovery_codes(user.id, &codes)
        .await
        .unwrap();
    assert_eq!(
        tx.mfa().count_unused_recovery_codes(user.id).await.unwrap(),
        10
    );
    let mut first = tx
        .mfa()
        .find_recovery_code(user.id, &hash_recovery_code("code-0"))
        .await
        .unwrap()
        .unwrap();
    first.use_code(fx.now()).unwrap();
    tx.mfa().update_recovery_code(&first).await.unwrap();
    assert_eq!(
        tx.mfa().count_unused_recovery_codes(user.id).await.unwrap(),
        9
    );
    tx.mfa()
        .replace_recovery_codes(user.id, &codes[..3])
        .await
        .unwrap();
    assert_eq!(
        tx.mfa().count_unused_recovery_codes(user.id).await.unwrap(),
        3
    );
    tx.mfa().delete_credential_and_codes(user.id).await.unwrap();
    assert!(tx.mfa().get_credential(user.id).await.unwrap().is_none());
    assert_eq!(
        tx.mfa().count_unused_recovery_codes(user.id).await.unwrap(),
        0
    );
}

#[tokio::test]
async fn settings_upsert_and_read_back() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let admin = fx.user("ada@example.com", Role::Admin);
    insert_user(&db, &admin).await;
    let mut tx = db.store.begin().await.unwrap();
    assert!(tx.settings().get_all().await.unwrap().is_empty());
    tx.settings()
        .put("signin.idle_timeout", "7200", admin.id, fx.now())
        .await
        .unwrap();
    tx.settings()
        .put("signin.idle_timeout", "3600", admin.id, fx.now())
        .await
        .unwrap();
    tx.settings()
        .put("password.online_breach_check", "true", admin.id, fx.now())
        .await
        .unwrap();
    let all = tx.settings().get_all().await.unwrap();
    assert_eq!(all["signin.idle_timeout"], "3600");
    assert_eq!(all.len(), 2);
}

// ---- guards -------------------------------------------------------------------------------

#[tokio::test]
async fn br_3_a_second_attempt_waits_for_the_first_to_finish_with_the_account_guard() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = fx.user("mia@example.com", Role::Member);
    insert_user(&db, &user).await;
    let store = Arc::new(db.store.clone());

    let mut first = store.begin().await.unwrap();
    let mut state = first
        .guards()
        .get_account_guard_for_update(user.id)
        .await
        .unwrap();
    assert_eq!(state, GuardState::default());

    let second_store = Arc::clone(&store);
    let second = tokio::spawn(async move {
        let mut tx = second_store.begin().await.unwrap();
        let seen = tx
            .guards()
            .get_account_guard_for_update(user.id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        seen
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !second.is_finished(),
        "the second attempt must block on the row lock"
    );

    state.record_failure(fx.now());
    first
        .guards()
        .put_account_guard(user.id, &state)
        .await
        .unwrap();
    first.commit().await.unwrap();
    let seen_by_second = tokio::time::timeout(Duration::from_secs(5), second)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        seen_by_second.failure_count, 1,
        "the second attempt sees the first's failure"
    );
}

#[tokio::test]
async fn br_3_5_unknown_email_guards_round_trip_and_the_table_stays_bounded() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let key = hash_token("signin-guard:ghost@example.com");
    let mut tx = db.store.begin().await.unwrap();
    let mut state = tx
        .guards()
        .get_unknown_guard_for_update(&key)
        .await
        .unwrap();
    assert_eq!(state, GuardState::default());
    state.record_failure(fx.now());
    tx.guards()
        .put_unknown_guard(&key, &state, fx.now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(
        tx.guards()
            .get_unknown_guard_for_update(&key)
            .await
            .unwrap()
            .failure_count,
        1
    );
    drop(tx);

    sqlx::query(
        "INSERT INTO tandem.unknown_sign_in_guards (key_hash, updated_at)
         SELECT 'filler-' || n, now() - make_interval(secs => n) FROM generate_series(1, 10050) AS n",
    )
    .execute(&db.superuser)
    .await
    .unwrap();
    let mut tx = db.store.begin().await.unwrap();
    tx.guards()
        .get_unknown_guard_for_update(&hash_token("another"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM unknown_sign_in_guards")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(
        count, 10_000,
        "the oldest rows beyond the limit were evicted"
    );
    let kept: i64 =
        sqlx::query_scalar("SELECT count(*) FROM unknown_sign_in_guards WHERE key_hash = $1")
            .bind(hash_token("another").as_str())
            .fetch_one(&db.app)
            .await
            .unwrap();
    assert_eq!(kept, 1, "the newest guard survives");
}

// ---- audit --------------------------------------------------------------------------------

#[tokio::test]
async fn audit_entries_round_trip_filter_and_page_newest_first() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let ada = fx.user("ada@example.com", Role::Admin);
    insert_user(&db, &ada).await;
    let mut tx = db.store.begin().await.unwrap();
    for i in 0..5 {
        fx.clock.advance(TimeDuration::seconds(1));
        let ty = if i % 2 == 0 {
            AuditEventType::UserInvited
        } else {
            AuditEventType::UserRoleChanged
        };
        let mut entry = fx.audit(ty, if i < 4 { Some(&ada) } else { None });
        entry.event.object_type = Some("user".to_owned());
        entry.event.object_id = Some(format!("u-{i}"));
        entry.event.request_id = Some(format!("req-{i}"));
        entry.event.source = Some("203.0.113.7".to_owned());
        tx.audit().append(&entry).await.unwrap();
    }
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    let first = tx
        .audit()
        .query(&AuditFilter::default(), None, 3)
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 3);
    assert_eq!(
        first.entries[0].event.object_id.as_deref(),
        Some("u-4"),
        "newest first"
    );
    assert_eq!(
        first.entries[0].event.details["note"], "grüße ✓",
        "details survive a round trip"
    );
    assert_eq!(first.entries[0].event.actor_label, "system");
    let cursor: AuditCursor = first.next.unwrap();
    let rest = tx
        .audit()
        .query(&AuditFilter::default(), Some(cursor), 10)
        .await
        .unwrap();
    assert_eq!(rest.entries.len(), 2);
    assert!(rest.next.is_none());

    let invited = AuditFilter {
        event_type: Some(AuditEventType::UserInvited),
        ..AuditFilter::default()
    };
    assert_eq!(
        tx.audit()
            .query(&invited, None, 10)
            .await
            .unwrap()
            .entries
            .len(),
        3
    );
    let by_actor = AuditFilter {
        actor_id: Some(ada.id),
        ..AuditFilter::default()
    };
    assert_eq!(
        tx.audit()
            .query(&by_actor, None, 10)
            .await
            .unwrap()
            .entries
            .len(),
        4
    );
    let window = AuditFilter {
        from: Some(fx.now() - TimeDuration::seconds(2)),
        to: Some(fx.now() - TimeDuration::seconds(1)),
        ..AuditFilter::default()
    };
    assert_eq!(
        tx.audit()
            .query(&window, None, 10)
            .await
            .unwrap()
            .entries
            .len(),
        2
    );
}

// ---- chat, outbox and jobs ----------------------------------------------------------------

#[tokio::test]
async fn br_13_messages_keep_insertion_order_for_equal_timestamps_and_allow_one_reply_each() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let owner = fx.user("mia@example.com", Role::Member);
    insert_user(&db, &owner).await;
    let session = fx.chat_session(&owner);
    let mut tx = db.store.begin().await.unwrap();
    tx.chat().insert_session(&session).await.unwrap();
    assert_eq!(
        tx.chat().get_session(session.id).await.unwrap().unwrap(),
        session
    );
    assert_eq!(
        tx.chat()
            .list_sessions_for_owner(owner.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        tx.chat()
            .list_sessions_for_owner(UserId::generate(&fx.clock, &fx.rng).unwrap())
            .await
            .unwrap()
            .is_empty()
    );

    let question = fx.message(&session, Author::User(owner.id), "hello", None);
    let reply = fx.message(
        &session,
        Author::Agent("mock-agent".to_owned()),
        "Received: hello",
        Some(question.id),
    );
    assert_eq!(
        question.created_at, reply.created_at,
        "same timestamp on purpose"
    );
    tx.chat()
        .insert_message(&reply)
        .await
        .expect_err("a reply cannot precede its parent (foreign key)");
    drop(tx);

    let mut tx = db.store.begin().await.unwrap();
    tx.chat().insert_session(&session).await.unwrap();
    tx.chat().insert_message(&question).await.unwrap();
    tx.chat().insert_message(&reply).await.unwrap();
    let listed = tx.chat().list_messages(session.id, 10).await.unwrap();
    assert_eq!(listed, vec![question.clone(), reply.clone()]);
    assert!(tx.chat().has_agent_reply_to(question.id).await.unwrap());
    assert!(!tx.chat().has_agent_reply_to(reply.id).await.unwrap());
    let second_reply = fx.message(
        &session,
        Author::Agent("mock-agent".to_owned()),
        "again",
        Some(question.id),
    );
    assert_eq!(
        tx.chat().get_message(reply.id).await.unwrap().unwrap(),
        reply
    );
    let conflict = tx.chat().insert_message(&second_reply).await;
    assert!(matches!(
        conflict,
        Err(StoreError::Conflict("chat_messages.in_reply_to"))
    ));
}

fn posted_event(
    fx: &Fx,
    owner: &User,
    session: &ChatSession,
    message: ChatMessageId,
    by_agent: bool,
) -> NewOutboxEvent {
    let _ = fx;
    NewOutboxEvent {
        payload: EventPayload::MessagePosted {
            chat_session_id: session.id,
            message_id: message,
            owner_id: owner.id,
            author_is_agent: by_agent,
        },
        trace_context: Some("00-abc-def-01".to_owned()),
    }
}

#[tokio::test]
async fn br_13_4_outbox_events_are_ordered_notify_on_commit_and_can_be_replayed_after_a_position() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let owner = fx.user("mia@example.com", Role::Member);
    let session = fx.chat_session(&owner);
    let message = ChatMessageId::generate(&fx.clock, &fx.rng).unwrap();
    let mut listener = PgListener::connect_with(&db.app).await.unwrap();
    listener.listen("tandem_events").await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    let first = tx
        .outbox()
        .append(
            &posted_event(&fx, &owner, &session, message, false),
            fx.now(),
        )
        .await
        .unwrap();
    let second = tx
        .outbox()
        .append(
            &posted_event(&fx, &owner, &session, message, true),
            fx.now(),
        )
        .await
        .unwrap();
    assert!(second > first);
    let early = tokio::time::timeout(Duration::from_millis(400), listener.recv()).await;
    assert!(
        early.is_err(),
        "no notification is delivered before the transaction commits"
    );
    tx.commit().await.unwrap();
    let notification = tokio::time::timeout(Duration::from_secs(5), listener.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notification.channel(), "tandem_events");
    assert_eq!(notification.payload(), first.0.to_string());

    let mut tx = db.store.begin().await.unwrap();
    let pending = tx.outbox().fetch_unpublished(10).await.unwrap();
    assert_eq!(
        pending.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![first, second],
        "in id order"
    );
    assert_eq!(pending[0].trace_context.as_deref(), Some("00-abc-def-01"));
    tx.outbox()
        .mark_published(&[first], fx.now())
        .await
        .unwrap();
    assert_eq!(tx.outbox().fetch_unpublished(10).await.unwrap().len(), 1);
    assert_eq!(
        tx.outbox().fetch_after(first, 10).await.unwrap().len(),
        1,
        "replay after a position"
    );
    assert_eq!(tx.outbox().fetch_after(second, 10).await.unwrap().len(), 0);
    assert_eq!(tx.outbox().lowest_retained_id().await.unwrap(), Some(first));
    assert_eq!(tx.outbox().latest_id().await.unwrap(), Some(second));
}

#[tokio::test]
async fn sc_3_jobs_are_leased_without_blocking_each_other_and_lapsed_leases_are_recovered() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let message = ChatMessageId::generate(&fx.clock, &fx.rng).unwrap();
    let (a, b) = (fx.job(message), fx.job(message));
    let mut future = fx.job(message);
    future.run_at = fx.now() + TimeDuration::hours(1);
    let mut tx = db.store.begin().await.unwrap();
    for job in [&a, &b, &future] {
        tx.jobs().enqueue(job).await.unwrap();
    }
    tx.commit().await.unwrap();

    let ttl = TimeDuration::seconds(60);
    let mut one = db.store.begin().await.unwrap();
    let mut two = db.store.begin().await.unwrap();
    let leased_one = one.jobs().lease_next(fx.now(), ttl).await.unwrap().unwrap();
    let leased_two = two
        .jobs()
        .lease_next(fx.now(), ttl)
        .await
        .unwrap()
        .expect("SKIP LOCKED gives the second job");
    assert_ne!(
        leased_one.id, leased_two.id,
        "two workers never lease the same job"
    );
    one.commit().await.unwrap();
    two.commit().await.unwrap();
    assert_eq!(
        (leased_one.status, leased_one.attempts),
        (JobStatus::Leased, 1)
    );
    assert_eq!(leased_one.lease_expires_at, Some(fx.now() + ttl));

    let mut tx = db.store.begin().await.unwrap();
    assert!(
        tx.jobs().lease_next(fx.now(), ttl).await.unwrap().is_none(),
        "the future job is not due"
    );
    assert_eq!(
        tx.jobs()
            .recover_lapsed_leases(fx.now() + TimeDuration::seconds(59))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        tx.jobs()
            .recover_lapsed_leases(fx.now() + TimeDuration::seconds(61))
            .await
            .unwrap(),
        2
    );
    let recovered = tx.jobs().get(leased_one.id).await.unwrap().unwrap();
    assert_eq!(recovered.status, JobStatus::Queued);
    assert_eq!(
        recovered.run_at,
        fx.now() + TimeDuration::seconds(61) + TimeDuration::seconds(5)
    );
    assert_eq!(recovered.last_error.as_deref(), Some("lease expired"));

    let mut job = tx
        .jobs()
        .lease_next(fx.now() + TimeDuration::minutes(2), ttl)
        .await
        .unwrap()
        .unwrap();
    job.complete().unwrap();
    tx.jobs().save(&job).await.unwrap();
    assert_eq!(
        tx.jobs().get(job.id).await.unwrap().unwrap().status,
        JobStatus::Done
    );
}

#[tokio::test]
async fn relay_lease_is_exclusive_until_released_or_dropped() {
    let db = TestDb::new().await;
    let first = RelayLease::try_acquire(&db.app)
        .await
        .unwrap()
        .expect("first caller leads");
    assert!(
        RelayLease::try_acquire(&db.app).await.unwrap().is_none(),
        "a second relay is refused"
    );
    first.release().await.unwrap();
    let again = RelayLease::try_acquire(&db.app)
        .await
        .unwrap()
        .expect("leadership is available again");
    drop(again);
    let mut regained = None;
    for _ in 0..20 {
        regained = RelayLease::try_acquire(&db.app).await.unwrap();
        if regained.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        regained.is_some(),
        "dropping the lease closes its connection and frees the lock"
    );
}

// ---- cleanup ------------------------------------------------------------------------------

#[tokio::test]
async fn cleanup_removes_expired_rows_and_never_touches_the_audit_log() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let admin = fx.user("ada@example.com", Role::Admin);
    insert_user(&db, &admin).await;
    let message = ChatMessageId::generate(&fx.clock, &fx.rng).unwrap();
    let mut tx = db.store.begin().await.unwrap();

    let mut ended = fx.session(&admin, "old-ended");
    ended.end(EndReason::SignedOut, fx.now());
    tx.sessions().insert(&ended).await.unwrap();
    tx.sessions()
        .insert(&fx.session(&admin, "still-active"))
        .await
        .unwrap();
    let mut used = PasswordReset::new(
        PasswordResetId::generate(&fx.clock, &fx.rng).unwrap(),
        admin.id,
        hash_token("r1"),
        admin.id,
        false,
        fx.now(),
    );
    used.mark_used(fx.now()).unwrap();
    tx.resets().insert(&used).await.unwrap();
    let mut done = fx.job(message);
    done.lease(fx.now(), TimeDuration::seconds(60)).unwrap();
    done.complete().unwrap();
    tx.jobs().enqueue(&done).await.unwrap();
    let event = posted_event(&fx, &admin, &fx.chat_session(&admin), message, false);
    let published = tx.outbox().append(&event, fx.now()).await.unwrap();
    tx.outbox()
        .mark_published(&[published], fx.now())
        .await
        .unwrap();
    tx.outbox().append(&event, fx.now()).await.unwrap();
    tx.audit()
        .append(&fx.audit(AuditEventType::AuthSetup, Some(&admin)))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Thirty-one days later everything above is past its retention except what is still in use.
    let later = fx.now() + TimeDuration::days(31);
    let report = db.store.cleanup(later).await.unwrap();
    assert_eq!(report.sessions, 1);
    assert_eq!(report.resets, 1);
    assert_eq!(report.jobs, 1);
    assert_eq!(
        report.outbox_events, 1,
        "only the published event; the unpublished one stays"
    );

    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(audit, 1, "the audit log is never pruned (SECURITY-14)");
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_sessions")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(sessions, 1, "the active sign-in stays");
    let outbox: i64 = sqlx::query_scalar("SELECT count(*) FROM outbox_events")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(outbox, 1);
    let again = db.store.cleanup(later).await.unwrap();
    assert_eq!(
        again,
        tandem_store_postgres::CleanupReport::default(),
        "a second run finds nothing"
    );
}
