//! The worker's loops and health server against a real PostgreSQL. Needs Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestDb;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use tandem_app::{Actor, RequestContext};
use tandem_config::{Config, Secret};
use tandem_core::chat::Author;
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::ids::{AuthSessionId, UserId};
use tandem_core::role::{Role, UserStatus};
use tandem_core::session::Scope;
use tandem_core::user::{CreatedVia, User};
use tandem_runtime::{Process, Services};
use tandem_support::{Clock, FakeClock, FakeRng};
use tandem_telemetry::{MetricsHandle, MetricsRegistry};
use tandem_worker::{WorkerHandle, WorkerOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TOKEN: &str = "a-long-enough-test-token-1234";

fn metrics() -> MetricsHandle {
    static HANDLE: OnceLock<MetricsHandle> = OnceLock::new();
    HANDLE
        .get_or_init(|| MetricsRegistry::new("test").unwrap().install().unwrap())
        .clone()
}

async fn services(db: &TestDb, dir: &Path) -> Services {
    sqlx::query(
        "INSERT INTO tandem.instance_settings (key, value, updated_at)
         VALUES ('internal.hash_profile', '19456:2:1', now()) ON CONFLICT DO NOTHING",
    )
    .execute(&db.superuser)
    .await
    .unwrap();
    let config = Config::load(&db.environment(dir)).unwrap();
    Services::start(config, Process::Worker).await.unwrap()
}

fn options(token: Option<&str>) -> WorkerOptions {
    WorkerOptions {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        metrics_token: token.map(Secret::new),
        poll: Duration::from_millis(100),
        stats_interval: Duration::from_millis(200),
        cleanup_first_delay: Duration::from_secs(3600),
        cleanup_interval: Duration::from_secs(3600),
        recover_interval: Duration::from_secs(1),
        drain_timeout: Duration::from_secs(10),
    }
}

async fn http_get(address: SocketAddr, path: &str, authorization: Option<&str>) -> (u16, String) {
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: worker\r\nConnection: close\r\n");
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status line");
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    (status, body)
}

async fn member_context(services: &Services) -> RequestContext {
    let clock = FakeClock::at_unix(1_800_000_000);
    let rng = FakeRng::new(99);
    let user = User {
        id: UserId::generate(&clock, &rng).unwrap(),
        email: Email::parse("mia@example.test").unwrap(),
        display_name: DisplayName::parse("Mia Member").unwrap(),
        role: Role::Member,
        status: UserStatus::Active,
        password_hash: "$argon2id$stub".to_owned(),
        password_changed_at: clock.now(),
        created_at: clock.now(),
        created_via: CreatedVia::Invitation,
        deactivated_at: None,
        deactivated_by: None,
        last_sign_in_at: None,
    };
    let mut tx = tandem_ports::Store::begin(&services.store).await.unwrap();
    tx.users().insert(&user).await.unwrap();
    tx.commit().await.unwrap();
    RequestContext::authenticated(
        Actor {
            user_id: user.id,
            display_name: "Mia Member".to_owned(),
            role: Role::Member,
            scope: Scope::Full,
            session_id: AuthSessionId::generate(&clock, &rng).unwrap(),
        },
        Some("req-worker-test".to_owned()),
        None,
    )
}

async fn start(services: &Services, token: Option<&str>) -> WorkerHandle {
    tandem_worker::start(services, metrics(), options(token))
        .await
        .unwrap()
}

async fn wait_for_reply(
    services: &Services,
    ctx: &RequestContext,
    chat: tandem_core::ids::ChatSessionId,
    expected: usize,
) {
    for _ in 0..200 {
        let messages = services.app.list_messages(ctx, chat).await.unwrap();
        if messages.len() >= expected {
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m.author, Author::Agent(_)))
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the agent never replied");
}

#[tokio::test]
async fn a_posted_message_gets_its_reply_through_the_relay_and_job_runner() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let services = services(&db, dir.path()).await;
    let ctx = member_context(&services).await;
    let chat = services
        .app
        .create_chat_session(&ctx, "Hello")
        .await
        .unwrap();
    services
        .app
        .post_message(&ctx, chat.id, "hello there")
        .await
        .unwrap();

    let handle = start(&services, None).await;
    wait_for_reply(&services, &ctx, chat.id, 2).await;
    let messages = services.app.list_messages(&ctx, chat.id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].in_reply_to, Some(messages[0].id));
    handle.stop().await;
}

#[tokio::test]
async fn only_one_of_two_workers_runs_the_relay_and_no_reply_is_duplicated() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let first_services = services(&db, dir.path()).await;
    let second_services = services(&db, dir.path()).await;
    let ctx = member_context(&first_services).await;
    let chat = first_services
        .app
        .create_chat_session(&ctx, "Race")
        .await
        .unwrap();
    for text in ["one", "two", "three"] {
        first_services
            .app
            .post_message(&ctx, chat.id, text)
            .await
            .unwrap();
    }
    let first = start(&first_services, None).await;
    let second = start(&second_services, None).await;
    wait_for_reply(&first_services, &ctx, chat.id, 6).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let messages = first_services
        .app
        .list_messages(&ctx, chat.id)
        .await
        .unwrap();
    assert_eq!(messages.len(), 6, "three questions, exactly three replies");
    let leaders: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND granted
         AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
    )
    .fetch_one(&db.superuser)
    .await
    .unwrap();
    assert_eq!(leaders, 1, "exactly one relay holds the lock");
    first.stop().await;
    second.stop().await;
}

#[tokio::test]
async fn readiness_is_503_once_shutdown_begins_and_health_stays_up() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let services = services(&db, dir.path()).await;
    let handle = start(&services, None).await;
    let address = handle.local_addr();

    assert_eq!(
        http_get(address, "/healthz", None).await,
        (200, "ok".to_owned())
    );
    let (status, body) = http_get(address, "/readyz", None).await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"ready\":true"));
    for probe in ["database", "schema", "vault", "relay_lag"] {
        assert!(body.contains(probe), "{probe} in {body}");
    }
    assert!(!body.contains("detail"));

    handle.begin_shutdown();
    let (status, body) = http_get(address, "/readyz", None).await;
    assert_eq!(status, 503);
    assert!(body.contains("\"shutting_down\":true"));
    assert_eq!(http_get(address, "/healthz", None).await.0, 200);
    handle.stop().await;
}

#[tokio::test]
async fn a_stuck_relay_makes_the_worker_not_ready() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let services = services(&db, dir.path()).await;
    sqlx::query(
        "INSERT INTO tandem.outbox_events (kind, payload, created_at)
         VALUES ('message_posted', '{}'::jsonb, now() - interval '5 minutes')",
    )
    .execute(&db.superuser)
    .await
    .unwrap();
    let readiness = tandem_telemetry::Readiness::new(
        vec![std::sync::Arc::new(tandem_worker::RelayLagProbe::new(
            services.store.clone(),
            tandem_worker::MAX_RELAY_LAG,
        ))],
        Duration::from_secs(1),
    );
    let report = readiness.report().await;
    assert!(!report.ready);
    assert_eq!(report.checks[0].name, "relay_lag");
}

#[tokio::test]
async fn metrics_are_404_without_a_token_and_need_the_right_bearer_with_one() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let services = services(&db, dir.path()).await;

    let disabled = start(&services, None).await;
    assert_eq!(
        http_get(
            disabled.local_addr(),
            "/metrics",
            Some(&format!("Bearer {TOKEN}"))
        )
        .await
        .0,
        404
    );
    disabled.stop().await;

    let enabled = start(&services, Some(TOKEN)).await;
    let address = enabled.local_addr();
    assert_eq!(http_get(address, "/metrics", None).await.0, 401);
    assert_eq!(
        http_get(address, "/metrics", Some("Basic abc")).await.0,
        401
    );
    assert_eq!(
        http_get(address, "/metrics", Some("Bearer wrong-token-value-000000"))
            .await
            .0,
        403
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    let (status, body) = http_get(address, "/metrics", Some(&format!("Bearer {TOKEN}"))).await;
    assert_eq!(status, 200);
    for name in ["outbox_pending", "job_queue_depth", "db_pool_connections"] {
        assert!(body.contains(name), "{name} missing");
    }
    enabled.stop().await;
}

#[tokio::test]
async fn stopping_waits_for_the_loops_and_releases_the_relay_lock() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let services = services(&db, dir.path()).await;
    let handle = start(&services, None).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.stop().await;
    let lease = tandem_store_postgres::RelayLease::try_acquire(services.store.pool())
        .await
        .unwrap();
    assert!(lease.is_some(), "the lock was released on shutdown");
}
