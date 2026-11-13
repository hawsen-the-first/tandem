//! U2 configuration storage against a real PostgreSQL 18: repositories, the unique active run, the
//! advisory lock between workers, grants, vault round trips and the whole test flow through two
//! workers. They need Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestDb;
use std::sync::Arc;
use tandem_app::testing::ScriptedProbes;
use tandem_app::{
    Actor, App, Dependencies, Integrations, JobOutcome, ModelInput, ProviderInput, RequestContext,
    SecretInput,
};
use tandem_core::config::{
    Connector, ConnectorAuth, ConnectorKind, ModelCapabilities, ModelDefaults, ModelName, Provider,
    ProviderKind, ProviderModel, ProviderName, RunStatus, SearchConfig, SearchKind, TestRun,
    TestStatus, TestTarget, ToolCalling,
};
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::ids::{AuthSessionId, ProviderId, ProviderModelId, TestRunId, UserId};
use tandem_core::outbound_url::OutboundUrl;
use tandem_core::role::{Role, UserStatus};
use tandem_core::sandbox_policy::{PolicyScope, SandboxPolicy};
use tandem_core::session::Scope;
use tandem_core::user::{CreatedVia, User};
use tandem_credentials::{ArgonHasher, HashProfile, HmacTotp};
use tandem_ports::{Locks, SecretChange, Store, StoreError, Vault};
use tandem_providers::MockProvider;
use tandem_store_postgres::{PgLocks, PgStore};
use tandem_support::{Clock, FakeClock, FakeRng, Rng, SystemRng};
use tandem_vault::EnvelopeVault;
use time::{Duration, OffsetDateTime};

const KEY_FILE: &str = "k1 current AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\n";

struct Fx {
    clock: FakeClock,
    rng: FakeRng,
}

impl Fx {
    fn new() -> Self {
        Self {
            clock: FakeClock::at_unix(1_800_000_000),
            rng: FakeRng::new(11),
        }
    }

    fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    fn user(&self) -> User {
        User {
            id: UserId::generate(&self.clock, &self.rng).unwrap(),
            email: Email::parse("ada@example.test").unwrap(),
            display_name: DisplayName::parse("Ada Admin").unwrap(),
            role: Role::Admin,
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

    fn provider(&self, by: UserId, name: &str) -> Provider {
        Provider {
            id: ProviderId::generate(&self.clock, &self.rng).unwrap(),
            kind: ProviderKind::OpenAiCompatible,
            name: ProviderName::parse(name).unwrap(),
            base_url: OutboundUrl::parse("http://localhost:11434/v1").unwrap(),
            key_saved_at: None,
            created_at: self.now(),
            created_by: by,
            updated_at: self.now(),
            updated_by: by,
        }
    }

    fn model(&self, provider: ProviderId, name: &str) -> ProviderModel {
        ProviderModel {
            id: ProviderModelId::generate(&self.clock, &self.rng).unwrap(),
            provider_id: provider,
            name: ModelName::parse(name).unwrap(),
            label: None,
            capabilities: ModelCapabilities::default(),
        }
    }

    fn run(&self, target: TestTarget, by: UserId) -> TestRun {
        TestRun::queued(
            TestRunId::generate(&self.clock, &self.rng).unwrap(),
            target,
            by,
            self.now(),
        )
    }
}

async fn admin(db: &TestDb, fx: &Fx) -> User {
    let user = fx.user();
    let mut tx = db.store.begin().await.unwrap();
    tx.users().insert(&user).await.unwrap();
    tx.commit().await.unwrap();
    user
}

#[tokio::test]
async fn providers_and_models_round_trip_keep_detected_values_and_cascade_on_delete() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let mut provider = fx.provider(user.id, "Local");
    provider.key_saved_at = Some(fx.now());
    let first = fx.model(provider.id, "llama3");
    let second = fx.model(provider.id, "qwen");

    let mut tx = db.store.begin().await.unwrap();
    tx.providers()
        .insert(&provider, Some(b"sealed-key"))
        .await
        .unwrap();
    tx.providers()
        .replace_models(provider.id, &[first.clone(), second.clone()])
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(
        tx.providers().get(provider.id).await.unwrap().unwrap(),
        provider
    );
    assert_eq!(
        tx.providers()
            .sealed_key(provider.id)
            .await
            .unwrap()
            .unwrap(),
        b"sealed-key"
    );
    let names: Vec<_> = tx
        .providers()
        .models(provider.id)
        .await
        .unwrap()
        .iter()
        .map(|model| model.name.as_str().to_owned())
        .collect();
    assert_eq!(names, ["llama3", "qwen"], "display order is kept");

    // A test records what it found; an admin edit replaces declared values only.
    tx.providers()
        .set_detected(first.id, ToolCalling::Supported, Some(32_768))
        .await
        .unwrap();
    let mut edited = first.clone();
    edited.name = ModelName::parse("llama3.1").unwrap();
    edited.capabilities.declared_context_tokens = Some(4096);
    let removed = tx
        .providers()
        .replace_models(provider.id, std::slice::from_ref(&edited))
        .await
        .unwrap();
    assert_eq!(removed, vec![second.id]);
    let stored = tx.providers().get_model(first.id).await.unwrap().unwrap();
    assert_eq!(stored.name.as_str(), "llama3.1");
    assert_eq!(
        stored.capabilities.detected_tool_calling,
        ToolCalling::Supported
    );
    assert_eq!(stored.capabilities.detected_context_tokens, Some(32_768));
    assert_eq!(stored.capabilities.declared_context_tokens, Some(4096));

    // Key changes: keep, clear, set.
    let mut changed = provider.clone();
    tx.providers()
        .update(&changed, SecretChange::Keep)
        .await
        .unwrap();
    assert!(
        tx.providers()
            .sealed_key(provider.id)
            .await
            .unwrap()
            .is_some()
    );
    changed.key_saved_at = None;
    tx.providers()
        .update(&changed, SecretChange::Clear)
        .await
        .unwrap();
    assert!(
        tx.providers()
            .sealed_key(provider.id)
            .await
            .unwrap()
            .is_none()
    );
    changed.key_saved_at = Some(fx.now());
    tx.providers()
        .update(&changed, SecretChange::Set(b"new".to_vec()))
        .await
        .unwrap();
    assert_eq!(
        tx.providers()
            .sealed_key(provider.id)
            .await
            .unwrap()
            .unwrap(),
        b"new"
    );
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    tx.providers().delete(provider.id).await.unwrap();
    assert!(
        tx.providers().all_models().await.unwrap().is_empty(),
        "models go with their provider"
    );
    assert!(matches!(
        tx.providers().delete(provider.id).await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn provider_and_model_names_are_unique_and_the_database_enforces_it() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let provider = fx.provider(user.id, "Local");
    {
        let mut tx = db.store.begin().await.unwrap();
        tx.providers().insert(&provider, None).await.unwrap();
        tx.commit().await.unwrap();
    }
    let mut tx = db.store.begin().await.unwrap();
    let clash = tx
        .providers()
        .insert(&fx.provider(user.id, "LOCAL"), None)
        .await;
    assert!(
        matches!(clash, Err(StoreError::Conflict("providers.name"))),
        "{clash:?}"
    );
    drop(tx);

    let mut tx = db.store.begin().await.unwrap();
    let twice = [fx.model(provider.id, "m"), fx.model(provider.id, "m")];
    let duplicate = tx.providers().replace_models(provider.id, &twice).await;
    assert!(
        matches!(duplicate, Err(StoreError::Conflict("provider_models.name"))),
        "{duplicate:?}"
    );
}

#[tokio::test]
async fn br_15_3_deleting_a_model_or_provider_clears_defaults_that_point_at_it() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let provider = fx.provider(user.id, "Local");
    let (a, b) = (fx.model(provider.id, "a"), fx.model(provider.id, "b"));
    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(tx.defaults().get().await.unwrap(), ModelDefaults::default());
    tx.providers().insert(&provider, None).await.unwrap();
    tx.providers()
        .replace_models(provider.id, &[a.clone(), b.clone()])
        .await
        .unwrap();
    tx.defaults()
        .save(
            &ModelDefaults {
                orchestrator: Some(a.id),
                agents: Some(b.id),
            },
            user.id,
            fx.now(),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    tx.providers()
        .replace_models(provider.id, std::slice::from_ref(&b))
        .await
        .unwrap();
    let defaults = tx.defaults().get().await.unwrap();
    assert_eq!(
        defaults.orchestrator, None,
        "the foreign key clears it even without the service"
    );
    assert_eq!(defaults.agents, Some(b.id));
    tx.providers().delete(provider.id).await.unwrap();
    assert_eq!(tx.defaults().get().await.unwrap(), ModelDefaults::default());
}

#[tokio::test]
async fn search_connector_and_policy_round_trip() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let mut tx = db.store.begin().await.unwrap();

    assert!(matches!(
        tx.search()
            .set_active(Some(SearchKind::Brave), user.id, fx.now())
            .await,
        Err(StoreError::NotFound)
    ));
    let config = SearchConfig {
        kind: SearchKind::Brave,
        base_url: None,
        key_saved_at: Some(fx.now()),
        updated_at: fx.now(),
        updated_by: user.id,
    };
    tx.search()
        .save(&config, SecretChange::Set(b"sealed".to_vec()))
        .await
        .unwrap();
    assert_eq!(
        tx.search().get(SearchKind::Brave).await.unwrap().unwrap(),
        config
    );
    assert_eq!(
        tx.search()
            .sealed_key(SearchKind::Brave)
            .await
            .unwrap()
            .unwrap(),
        b"sealed"
    );
    tx.search()
        .set_active(Some(SearchKind::Brave), user.id, fx.now())
        .await
        .unwrap();
    assert_eq!(tx.search().active().await.unwrap(), Some(SearchKind::Brave));
    tx.search()
        .set_active(None, user.id, fx.now())
        .await
        .unwrap();
    assert_eq!(tx.search().active().await.unwrap(), None);
    assert_eq!(
        tx.search().list().await.unwrap().len(),
        1,
        "inactive settings are kept"
    );

    let connector = Connector {
        kind: ConnectorKind::Github,
        auth: ConnectorAuth::Token,
        secret_saved_at: fx.now(),
        account_login: None,
        token_expires_at: None,
        updated_at: fx.now(),
        updated_by: user.id,
    };
    tx.connectors()
        .save(&connector, b"sealed-token")
        .await
        .unwrap();
    let expires = fx.now() + Duration::days(30);
    tx.connectors()
        .set_identity(ConnectorKind::Github, Some("octocat"), Some(expires))
        .await
        .unwrap();
    let stored = tx
        .connectors()
        .get(ConnectorKind::Github)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.account_login.as_deref(), Some("octocat"));
    assert_eq!(stored.token_expires_at, Some(expires));
    tx.connectors()
        .save(&connector, b"replacement")
        .await
        .unwrap();
    let replaced = tx
        .connectors()
        .get(ConnectorKind::Github)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        replaced.account_login, None,
        "a replaced token drops the old identity"
    );
    assert_eq!(
        tx.connectors()
            .sealed_secret(ConnectorKind::Github)
            .await
            .unwrap()
            .unwrap(),
        b"replacement"
    );
    tx.connectors().delete(ConnectorKind::Github).await.unwrap();
    assert!(matches!(
        tx.connectors().delete(ConnectorKind::Github).await,
        Err(StoreError::NotFound)
    ));
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    assert!(
        tx.policies()
            .get(PolicyScope::Instance)
            .await
            .unwrap()
            .is_none()
    );
    let mut policy = SandboxPolicy::factory_default();
    policy.limits.wall_clock_seconds = 900;
    policy.network =
        tandem_core::sandbox_policy::NetworkRule::allow_hosts(["*.example.com", "api.test"])
            .unwrap();
    policy.version = 2;
    tx.policies()
        .save(&policy, 1, user.id, fx.now())
        .await
        .unwrap();
    assert_eq!(
        tx.policies()
            .get(PolicyScope::Instance)
            .await
            .unwrap()
            .unwrap(),
        policy
    );
    let mut newer = policy.clone();
    newer.version = 3;
    tx.policies()
        .save(&newer, 2, user.id, fx.now())
        .await
        .unwrap();
    let stale = tx.policies().save(&newer, 2, user.id, fx.now()).await;
    assert!(
        matches!(stale, Err(StoreError::Conflict("sandbox_policies.version"))),
        "{stale:?}"
    );
    let first_save_again = tx.policies().save(&policy, 1, user.id, fx.now()).await;
    assert!(
        matches!(first_save_again, Err(StoreError::Conflict(_))),
        "version 1 is gone"
    );
}

#[tokio::test]
async fn only_one_run_per_target_can_be_active_and_the_old_ones_are_pruned() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let target = TestTarget::Runtime;

    let mut first = fx.run(target, user.id);
    {
        let mut tx = db.store.begin().await.unwrap();
        tx.test_runs().insert(&first).await.unwrap();
        tx.commit().await.unwrap();
    }
    let mut tx = db.store.begin().await.unwrap();
    let second = tx.test_runs().insert(&fx.run(target, user.id)).await;
    assert!(
        matches!(second, Err(StoreError::Conflict("config_test_runs.active"))),
        "{second:?}"
    );
    drop(tx);

    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(
        tx.test_runs().active_for(target).await.unwrap().unwrap().id,
        first.id
    );
    assert!(
        tx.test_runs()
            .active_for(TestTarget::Search(SearchKind::Brave))
            .await
            .unwrap()
            .is_none()
    );
    first.start(fx.now()).unwrap();
    first
        .record(tandem_core::config::ProbeResult::new(
            tandem_core::config::ProbeSubject::Runtime,
            TestStatus::Ok,
        ))
        .unwrap();
    first.finish(fx.now()).unwrap();
    tx.test_runs().save(&first).await.unwrap();
    assert_eq!(tx.test_runs().get(first.id).await.unwrap().unwrap(), first);
    tx.commit().await.unwrap();

    // Finished: the target is free again, and the latest finished run is the first.
    let mut tx = db.store.begin().await.unwrap();
    assert_eq!(
        tx.test_runs()
            .latest_finished(target)
            .await
            .unwrap()
            .unwrap()
            .id,
        first.id
    );
    for _ in 0..12 {
        fx.clock.advance(Duration::minutes(1));
        let mut run = fx.run(target, user.id);
        tx.test_runs().insert(&run).await.unwrap();
        run.start(fx.now()).unwrap();
        run.finish(fx.now()).unwrap();
        tx.test_runs().save(&run).await.unwrap();
    }
    let deleted = tx
        .test_runs()
        .prune(10, fx.now() - Duration::days(30))
        .await
        .unwrap();
    assert_eq!(deleted, 3, "13 finished runs, keep the newest 10");
    let old = fx.now() + Duration::days(60);
    let deleted_old = tx.test_runs().prune(10, old).await.unwrap();
    assert_eq!(
        deleted_old, 9,
        "past 30 days only the latest finished run survives"
    );
    assert!(
        tx.test_runs()
            .latest_finished(target)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn queued_runs_older_than_a_cutoff_are_found() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let mut tx = db.store.begin().await.unwrap();
    let run = fx.run(TestTarget::Runtime, user.id);
    tx.test_runs().insert(&run).await.unwrap();
    assert!(
        tx.test_runs()
            .queued_before(fx.now())
            .await
            .unwrap()
            .is_empty()
    );
    let later = fx.now() + Duration::seconds(121);
    assert_eq!(
        tx.test_runs().queued_before(later).await.unwrap()[0].id,
        run.id
    );
}

#[tokio::test]
async fn two_workers_cannot_hold_the_same_target_lock_and_a_dropped_guard_frees_it() {
    let db = TestDb::new().await;
    let worker_one = PgLocks::new(db.app.clone());
    let worker_two = PgLocks::new(db.app.clone());
    let key = TestTarget::Runtime.key();

    let held = worker_one
        .try_lock(&key)
        .await
        .unwrap()
        .expect("first worker takes it");
    assert!(
        worker_two.try_lock(&key).await.unwrap().is_none(),
        "the second does not"
    );
    assert!(
        worker_two.try_lock("search:brave").await.unwrap().is_some(),
        "other targets are independent"
    );
    held.release().await;
    let again = worker_two
        .try_lock(&key)
        .await
        .unwrap()
        .expect("free after release");

    // A crashed worker never releases: dropping the guard closes its connection, which frees it.
    drop(again);
    let mut taken = None;
    for _ in 0..40 {
        taken = worker_one.try_lock(&key).await.unwrap();
        if taken.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        taken.is_some(),
        "the lock was freed when the guard was dropped"
    );
}

#[tokio::test]
async fn br_20_1_sealed_secrets_round_trip_through_the_database_bound_to_their_row() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let rng: Arc<dyn Rng> = Arc::new(SystemRng);
    let vault = EnvelopeVault::from_key_text(KEY_FILE, rng).unwrap();
    let mut provider = fx.provider(user.id, "Local");
    provider.key_saved_at = Some(fx.now());
    let sealed = vault
        .seal(
            "provider_api_key",
            &provider.id.to_string(),
            b"sk-secret-value",
        )
        .unwrap();
    let mut tx = db.store.begin().await.unwrap();
    tx.providers()
        .insert(&provider, Some(&sealed))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut tx = db.store.begin().await.unwrap();
    let stored = tx
        .providers()
        .sealed_key(provider.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !stored
            .windows(15)
            .any(|window| window == b"sk-secret-value")
    );
    assert_eq!(
        vault
            .open("provider_api_key", &provider.id.to_string(), &stored)
            .unwrap(),
        b"sk-secret-value"
    );
    assert!(
        vault
            .open("provider_api_key", "another-row", &stored)
            .is_err()
    );
    assert!(
        vault
            .open("search_api_key", &provider.id.to_string(), &stored)
            .is_err()
    );
}

#[tokio::test]
async fn se_26_the_app_role_manages_configuration_but_cannot_change_the_audit_log() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let provider = fx.provider(user.id, "Local");
    let mut tx = db.store.begin().await.unwrap();
    tx.providers().insert(&provider, None).await.unwrap();
    tx.providers().delete(provider.id).await.unwrap();
    tx.commit().await.unwrap();
    for statement in [
        "UPDATE audit_entries SET outcome = 'success'",
        "DELETE FROM audit_entries",
        "TRUNCATE providers",
        "DROP TABLE providers",
    ] {
        assert!(
            sqlx::query(statement).execute(&db.app).await.is_err(),
            "the app role must not be able to: {statement}"
        );
    }
    sqlx::query("DELETE FROM config_test_runs")
        .execute(&db.app)
        .await
        .unwrap();
}

fn app_with(db: &TestDb, probes: &ScriptedProbes, clock: &FakeClock) -> App {
    let rng: Arc<dyn Rng> = Arc::new(SystemRng);
    let vault = Arc::new(EnvelopeVault::from_key_text(KEY_FILE, Arc::clone(&rng)).unwrap());
    App::new(Dependencies {
        store: Arc::new(PgStore::new(db.app.clone())),
        clock: Arc::new(clock.clone()),
        rng: Arc::clone(&rng),
        hasher: Arc::new(
            ArgonHasher::new(
                HashProfile::MINIMUM,
                2,
                std::time::Duration::from_secs(10),
                Arc::clone(&rng),
            )
            .unwrap(),
        ),
        breach: Arc::new(tandem_app::testing::FakeBreachChecker::default()),
        totp: Arc::new(HmacTotp::new(Arc::clone(&rng))),
        vault,
        provider: Arc::new(MockProvider::new()),
        issuer: "Tandem".to_owned(),
        integrations: Integrations {
            locks: Arc::new(PgLocks::new(db.app.clone())),
            provider_probe: Arc::new(probes.clone()),
            search_probe: Arc::new(probes.clone()),
            connector_probe: Arc::new(probes.clone()),
            runtime_probe: Arc::new(probes.clone()),
        },
    })
}

fn admin_context(user: &User) -> RequestContext {
    RequestContext::authenticated(
        Actor {
            user_id: user.id,
            display_name: user.display_name.as_str().to_owned(),
            role: Role::Admin,
            scope: Scope::Full,
            session_id: AuthSessionId::generate(&FakeClock::at_unix(1), &FakeRng::new(3)).unwrap(),
        },
        Some("req-config".to_owned()),
        None,
    )
}

#[tokio::test]
async fn a_started_test_runs_once_across_two_workers_and_stores_its_results() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let probes = ScriptedProbes::default();
    let (app_one, app_two) = (
        app_with(&db, &probes, &fx.clock),
        app_with(&db, &probes, &fx.clock),
    );
    let ctx = admin_context(&user);

    let view = app_one
        .create_provider(
            &ctx,
            ProviderInput {
                name: "Local".to_owned(),
                base_url: "http://localhost:11434/v1".to_owned(),
                key: SecretInput::Set(tandem_support::Secret::new("sk-flow-key")),
                models: vec![ModelInput {
                    id: None,
                    name: "llama3".to_owned(),
                    label: String::new(),
                    declared_tool_calling: None,
                    declared_context_tokens: None,
                }],
            },
        )
        .await
        .unwrap();
    let target = TestTarget::Provider(view.provider.id);
    let run = app_one.start_test(&ctx, target).await.unwrap();
    assert!(matches!(
        app_two.start_test(&ctx, target).await,
        Err(tandem_app::AppError::AlreadyRunning { run_id }) if run_id == run.id
    ));

    let ttl = Duration::seconds(60);
    let (one, two) = tokio::join!(app_one.process_next_job(ttl), app_two.process_next_job(ttl));
    let outcomes = [one.unwrap(), two.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                JobOutcome::Completed {
                    kind: "config_test"
                }
            ))
            .count(),
        1,
        "{outcomes:?}"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, JobOutcome::NoJob))
            .count(),
        1
    );
    assert_eq!(probes.calls.lock().unwrap().provider_runs, 1);
    assert_eq!(
        probes.calls.lock().unwrap().provider_keys,
        vec![Some("sk-flow-key".to_owned())]
    );

    let finished = app_two.get_test_run(&ctx, run.id).await.unwrap();
    assert_eq!(finished.status, RunStatus::Succeeded);
    assert_eq!(finished.results.len(), 2);
    let shown = app_two.get_provider(&ctx, view.provider.id).await.unwrap();
    assert_eq!(
        shown.models[0].model.capabilities.detected_tool_calling,
        ToolCalling::Supported
    );
    assert_eq!(
        shown.models[0].model.capabilities.detected_context_tokens,
        Some(32_768)
    );

    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_entries WHERE event_type = 'config.test.run'",
    )
    .fetch_one(&db.app)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    // The target can be tested again.
    app_one.start_test(&ctx, target).await.unwrap();
}

#[tokio::test]
async fn a_target_locked_by_another_worker_is_retried_not_run_twice() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let probes = ScriptedProbes::default();
    let app = app_with(&db, &probes, &fx.clock);
    let ctx = admin_context(&user);

    let run = app.start_test(&ctx, TestTarget::Runtime).await.unwrap();
    let other_worker = PgLocks::new(db.app.clone());
    let held = other_worker
        .try_lock(&TestTarget::Runtime.key())
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(
        app.process_next_job(Duration::seconds(60)).await.unwrap(),
        JobOutcome::Completed { .. }
    ));
    assert_eq!(
        app.get_test_run(&ctx, run.id).await.unwrap().status,
        RunStatus::Queued
    );
    assert_eq!(probes.calls.lock().unwrap().runtime_runs, 0);

    held.release().await;
    fx.clock.advance(Duration::seconds(6));
    app.process_next_job(Duration::seconds(60)).await.unwrap();
    assert_eq!(
        app.get_test_run(&ctx, run.id).await.unwrap().status,
        RunStatus::Succeeded
    );
    assert_eq!(probes.calls.lock().unwrap().runtime_runs, 1);
}

#[tokio::test]
async fn deleting_a_provider_through_the_service_clears_its_defaults_in_one_transaction() {
    let db = TestDb::new().await;
    let fx = Fx::new();
    let user = admin(&db, &fx).await;
    let probes = ScriptedProbes::default();
    let app = app_with(&db, &probes, &fx.clock);
    let ctx = admin_context(&user);
    let view = app
        .create_provider(
            &ctx,
            ProviderInput {
                name: "Local".to_owned(),
                base_url: "http://localhost:11434/v1".to_owned(),
                key: SecretInput::Keep,
                models: vec![ModelInput {
                    id: None,
                    name: "a".to_owned(),
                    label: String::new(),
                    declared_tool_calling: None,
                    declared_context_tokens: None,
                }],
            },
        )
        .await
        .unwrap();
    let model = view.models[0].model.id;
    app.set_defaults(
        &ctx,
        tandem_app::DefaultsInput {
            orchestrator: Some(model),
            agents: Some(model),
        },
    )
    .await
    .unwrap();
    app.delete_provider(&ctx, view.provider.id).await.unwrap();
    let defaults = app.get_defaults(&ctx).await.unwrap();
    assert!(defaults.orchestrator.is_none() && defaults.agents.is_none());
    let cleared: String = sqlx::query_scalar(
        "SELECT details->>'cleared_defaults' FROM audit_entries WHERE event_type = 'config.provider.deleted'",
    )
    .fetch_one(&db.app)
    .await
    .unwrap();
    assert_eq!(cleared, "orchestrator,agents");
}
