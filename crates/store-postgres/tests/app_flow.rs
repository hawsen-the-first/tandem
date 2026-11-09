//! The real application over the real PostgreSQL adapter: the skeleton's whole journey.
//! Needs Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestDb;
use std::sync::Arc;
use std::time::Duration;
use tandem_app::testing::FakeBreachChecker;
use tandem_app::{
    App, Dependencies, JobOutcome, RequestContext, SecondFactor, SetupInput, SignInInput,
    SignInOutcome,
};
use tandem_core::chat::Author;
use tandem_core::role::Role;
use tandem_credentials::{ArgonHasher, HashProfile, HmacTotp};
use tandem_ports::{TotpService, Vault};
use tandem_providers::MockProvider;
use tandem_support::{Clock, FakeClock, Rng, SystemRng};
use tandem_vault::EnvelopeVault;

const PASSWORD: &str = "Correct-Horse-7";
const KEY_FILE: &str = "k1 current AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\n";

fn context(token_actor: tandem_app::Actor) -> RequestContext {
    RequestContext::authenticated(
        token_actor,
        Some("req-flow".to_owned()),
        Some("203.0.113.9".to_owned()),
    )
}

/// Find the code the authenticator would show now, by asking the verifier.
fn code_now(totp: &HmacTotp, secret: &[u8], at: time::OffsetDateTime) -> String {
    (0..1_000_000)
        .map(|n| format!("{n:06}"))
        .find(|candidate| totp.verify(secret, candidate, at).is_some())
        .expect("a valid code exists")
}

#[tokio::test]
async fn a_team_signs_up_chats_and_the_agent_replies_over_postgres() {
    let db = TestDb::new().await;
    let clock = FakeClock::at_unix(1_800_000_000);
    let rng: Arc<dyn Rng> = Arc::new(SystemRng);
    let totp = Arc::new(HmacTotp::new(Arc::clone(&rng)));
    let vault = Arc::new(EnvelopeVault::from_key_text(KEY_FILE, Arc::clone(&rng)).unwrap());
    let app = App::new(Dependencies {
        store: Arc::new(db.store.clone()),
        clock: Arc::new(clock.clone()),
        rng: Arc::clone(&rng),
        hasher: Arc::new(
            ArgonHasher::new(
                HashProfile::MINIMUM,
                2,
                Duration::from_secs(10),
                Arc::clone(&rng),
            )
            .unwrap(),
        ),
        breach: Arc::new(FakeBreachChecker::default()),
        totp: totp.clone(),
        vault: vault.clone(),
        provider: Arc::new(MockProvider::new()),
        issuer: "Tandem".to_owned(),
    });
    let anonymous = RequestContext::anonymous(Some("req-flow".to_owned()), None);

    // First-run setup, then MFA enrolment.
    assert!(app.setup_available().await.unwrap());
    let setup = app
        .first_run_setup(
            &anonymous,
            SetupInput {
                display_name: "Ada Admin".to_owned(),
                email: "ada@example.test".to_owned(),
                password: PASSWORD.to_owned(),
            },
        )
        .await
        .unwrap();
    assert!(!app.setup_available().await.unwrap());
    let enrolling = context(app.resolve_session(&setup.token).await.unwrap().actor);
    app.mfa_start_enrolment(&enrolling).await.unwrap();
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT secret_sealed FROM tandem.mfa_credentials")
        .fetch_one(&db.superuser)
        .await
        .unwrap();
    let secret = vault
        .open("mfa_secret", &setup.user.id.to_string(), &sealed)
        .unwrap();
    let recovery = app
        .mfa_confirm_enrolment(&enrolling, &code_now(&totp, &secret, Clock::now(&clock)))
        .await
        .unwrap();
    assert!(!recovery.is_empty());

    // Sign in again: the password alone asks for the second factor.
    clock.advance(time::Duration::seconds(90));
    let credentials = |second_factor| SignInInput {
        email: "ada@example.test".to_owned(),
        password: PASSWORD.to_owned(),
        second_factor,
    };
    assert!(matches!(
        app.sign_in(&anonymous, credentials(None)).await.unwrap(),
        SignInOutcome::MfaRequired
    ));
    let code = code_now(&totp, &secret, Clock::now(&clock));
    let SignInOutcome::Success(admin_session) = app
        .sign_in(&anonymous, credentials(Some(SecondFactor::Totp(code))))
        .await
        .unwrap()
    else {
        panic!("expected a sign-in");
    };
    let admin = context(
        app.resolve_session(&admin_session.token)
            .await
            .unwrap()
            .actor,
    );

    // Invite a member, who accepts and chats.
    let invitation = app
        .invite(&admin, "mia@example.test", Role::Member)
        .await
        .unwrap();
    let member_session = app
        .accept_invitation(&anonymous, &invitation.token, "Mia Member", PASSWORD)
        .await
        .unwrap();
    let member = context(
        app.resolve_session(&member_session.token)
            .await
            .unwrap()
            .actor,
    );
    let chat = app.create_chat_session(&member, "Hello").await.unwrap();
    let question = app.post_message(&member, chat.id, "hello").await.unwrap();

    // The relay turns the event into a job; the worker runs it and the agent replies.
    assert_eq!(app.relay_outbox_batch(50).await.unwrap(), 1);
    let outcome = app
        .process_next_job(time::Duration::seconds(60))
        .await
        .unwrap();
    assert!(
        matches!(outcome, JobOutcome::Completed { .. }),
        "{outcome:?}"
    );
    let messages = app.list_messages(&member, chat.id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id, question.id);
    assert!(matches!(messages[1].author, Author::Agent(_)));
    assert_eq!(messages[1].in_reply_to, Some(question.id));
    assert!(matches!(
        app.process_next_job(time::Duration::seconds(60))
            .await
            .unwrap(),
        JobOutcome::NoJob
    ));

    // Another member's session is not visible to the admin.
    assert!(app.list_messages(&admin, chat.id).await.is_err());

    // The audit trail recorded the journey, and the app role cannot rewrite it.
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert!(
        audited >= 4,
        "setup, MFA, sign-in and invitation events: {audited}"
    );
    assert!(
        sqlx::query("UPDATE audit_entries SET outcome = 'tampered'")
            .execute(&db.app)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_entries")
            .execute(&db.app)
            .await
            .is_err()
    );
}
