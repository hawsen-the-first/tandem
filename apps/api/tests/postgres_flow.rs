//! The whole service over a real PostgreSQL and real sockets: start-up guards, setup with real
//! TOTP, invitations, chat, the event feed over WebSocket (wake-up, replay, limits, Origin),
//! readiness with real probes and the audit trail. Needs Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
#[path = "postgres_common/mod.rs"]
mod pg;

use axum::Router;
use axum::http::StatusCode;
use common::{Call, Reply, delete, get, post, send_to};
use futures_util::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha1::Sha1;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tandem_api::routes::ws::spawn_listener;
use tandem_api::server::{StateInputs, build, serve};
use tandem_api::state::AppState;
use tandem_bootstrap::{DatabaseProbe, Process, SchemaProbe, Services, Shutdown, VaultProbe};
use tandem_config::Config;
use tandem_telemetry::{MetricsHandle, MetricsRegistry, Readiness, ReadinessProbe};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const PASSWORD: &str = "Correct-Horse-7";

fn metrics() -> MetricsHandle {
    static HANDLE: OnceLock<MetricsHandle> = OnceLock::new();
    HANDLE
        .get_or_init(|| MetricsRegistry::new("test").unwrap().install().unwrap())
        .clone()
}

fn totp(secret_base32: &str, unix_seconds: i64) -> String {
    let secret = data_encoding::BASE32_NOPAD
        .decode(secret_base32.as_bytes())
        .unwrap();
    let mut mac = Hmac::<Sha1>::new_from_slice(&secret).unwrap();
    mac.update(&u64::try_from(unix_seconds / 30).unwrap().to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[digest.len() - 1] & 0x0F);
    let truncated = u32::from_be_bytes([
        digest[offset] & 0x7F,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    format!("{:06}", truncated % 1_000_000)
}

struct Stack {
    router: Router,
    state: AppState,
    services: Services,
    origin: String,
    address: std::net::SocketAddr,
    shutdown: Shutdown,
    _dir: tempfile::TempDir,
    _db: pg::TestDb,
}

impl Stack {
    async fn start() -> Self {
        let db = pg::TestDb::new().await;
        sqlx::query(
            "INSERT INTO tandem.instance_settings (key, value, updated_at)
             VALUES ('internal.hash_profile', '19456:2:1', now()) ON CONFLICT DO NOTHING",
        )
        .execute(&db.superuser)
        .await
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&db.environment(dir.path())).unwrap();
        let origin = config.public_origin();
        let services = Services::start(config, Process::Api).await.unwrap();
        let shutdown = Shutdown::new();
        let pool = services.store.pool().clone();
        let probes: Vec<Arc<dyn ReadinessProbe>> = vec![
            Arc::new(DatabaseProbe::new(pool.clone())),
            Arc::new(SchemaProbe::new(pool.clone())),
            Arc::new(VaultProbe::new(
                services.store.clone(),
                services.vault.clone(),
            )),
        ];
        let (router, state) = build(StateInputs {
            app: services.app.clone(),
            config: services.config.clone(),
            readiness: Readiness::new(probes, Duration::from_secs(1)),
            metrics: metrics(),
            shutdown: shutdown.clone(),
            web_dir: None,
        })
        .unwrap();
        spawn_listener(pool, state.hub.clone(), shutdown.subscribe());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(serve(listener, router.clone(), shutdown.clone()));
        Self {
            router,
            state,
            services,
            origin,
            address,
            shutdown,
            _dir: dir,
            _db: db,
        }
    }

    async fn send(&self, call: Call) -> Reply {
        send_to(&self.router, call).await
    }

    fn signed_in(&self, call: Call, user: &Person) -> Call {
        call.cookie(&user.cookie)
            .csrf(&user.csrf)
            .origin_of(&self.origin)
    }

    async fn identify(&self, cookie: &str) -> Person {
        let reply = self.send(get("/api/auth/me").cookie(cookie)).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
        let body = reply.json();
        Person {
            cookie: cookie.to_owned(),
            csrf: body["csrf_token"].as_str().unwrap().to_owned(),
        }
    }
}

struct Person {
    cookie: String,
    csrf: String,
}

async fn open_feed(
    stack: &Stack,
    cookie: Option<&str>,
    origin: Option<&str>,
    query: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Error,
> {
    let url = format!("ws://{}/api/events{query}", stack.address);
    let mut request = url.into_client_request().unwrap();
    if let Some(cookie) = cookie {
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
    }
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

async fn next_json<S>(socket: &mut S, wait: Duration) -> Option<Value>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match tokio::time::timeout(wait, socket.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => return serde_json::from_str(&text).ok(),
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {}
            _ => return None,
        }
    }
}

fn handshake_status(
    result: &Result<impl Sized, tokio_tungstenite::tungstenite::Error>,
) -> Option<u16> {
    match result {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            Some(response.status().as_u16())
        }
        _ => None,
    }
}

#[tokio::test]
async fn the_whole_service_works_end_to_end_over_postgres() {
    let stack = Stack::start().await;

    // Readiness reflects the real probes.
    let ready = stack.send(get("/readyz")).await;
    assert_eq!(ready.status, StatusCode::OK, "{}", ready.text);
    assert_eq!(ready.json()["checks"].as_array().unwrap().len(), 3);

    // First-run setup, then MFA enrolment with a real TOTP code.
    let setup = stack
        .send(post("/api/setup").origin_of(&stack.origin).json(&json!({
            "display_name": "Ada Admin", "email": "ada@example.test", "password": PASSWORD
        })))
        .await;
    assert_eq!(setup.status, StatusCode::OK, "{}", setup.text);
    let enrolling = stack.identify(&setup.session_cookie().unwrap()).await;
    let enrol = stack
        .send(stack.signed_in(post("/api/auth/mfa/enrol"), &enrolling))
        .await;
    assert_eq!(enrol.status, StatusCode::OK, "{}", enrol.text);
    let secret = enrol.json()["secret_base32"].as_str().unwrap().to_owned();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let confirm = stack
        .send(
            stack
                .signed_in(post("/api/auth/mfa/confirm"), &enrolling)
                .json(&json!({"code": totp(&secret, now)})),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::OK, "{}", confirm.text);
    assert_eq!(
        confirm.json()["recovery_codes"].as_array().unwrap().len(),
        10
    );
    let admin = enrolling;
    assert_eq!(
        stack.send(get("/api/setup")).await.json()["available"],
        false
    );

    // Invite a member, who accepts and chats.
    let invite = stack
        .send(
            stack
                .signed_in(post("/api/admin/invitations"), &admin)
                .json(&json!({"email": "mia@example.test", "role": "member"})),
        )
        .await;
    assert_eq!(invite.status, StatusCode::CREATED, "{}", invite.text);
    let token = invite.json()["link"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let accept = stack
        .send(
            post("/api/invitations/accept")
                .origin_of(&stack.origin)
                .json(&json!({"token": token, "display_name": "Mia Member", "password": PASSWORD})),
        )
        .await;
    assert_eq!(accept.status, StatusCode::OK, "{}", accept.text);
    let member = stack.identify(&accept.session_cookie().unwrap()).await;
    let session = stack
        .send(
            stack
                .signed_in(post("/api/chat/sessions"), &member)
                .json(&json!({"title": "Skeleton"})),
        )
        .await
        .json();
    let session_id = session["id"].as_str().unwrap().to_owned();

    // The feed refuses a foreign Origin, no cookie, and works with both.
    let foreign = open_feed(&stack, Some(&member.cookie), Some("https://evil.test"), "").await;
    assert_eq!(handshake_status(&foreign), Some(403));
    let anonymous = open_feed(&stack, None, Some(&stack.origin), "").await;
    assert_eq!(handshake_status(&anonymous), Some(401));
    let no_origin = open_feed(&stack, Some(&member.cookie), None, "").await;
    assert_eq!(handshake_status(&no_origin), Some(403));

    let mut feed = open_feed(&stack, Some(&member.cookie), Some(&stack.origin), "")
        .await
        .unwrap();
    let ready_frame = next_json(&mut feed, Duration::from_secs(3)).await.unwrap();
    assert_eq!(ready_frame["type"], "ready");
    let mut admin_feed = open_feed(&stack, Some(&admin.cookie), Some(&stack.origin), "")
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut admin_feed, Duration::from_secs(3))
            .await
            .unwrap()["type"],
        "ready"
    );

    // Post a message: the feed wakes (NOTIFY) with the message event.
    let posted = stack
        .send(
            stack
                .signed_in(
                    post(&format!("/api/chat/sessions/{session_id}/messages")),
                    &member,
                )
                .json(&json!({"body": "hello team"})),
        )
        .await;
    assert_eq!(posted.status, StatusCode::CREATED, "{}", posted.text);
    let first = next_json(&mut feed, Duration::from_secs(5)).await.unwrap();
    assert_eq!(first["type"], "event");
    assert_eq!(first["kind"], "message.posted");
    assert_eq!(first["author_is_agent"], false);
    assert_eq!(first["chat_session_id"], session_id);

    // The worker's steps (the loops are tested in the worker crate): relay, run the job, reply.
    assert_eq!(stack.services.app.relay_outbox_batch(100).await.unwrap(), 1);
    let outcome = stack
        .services
        .app
        .process_next_job(time::Duration::seconds(60))
        .await
        .unwrap();
    assert!(
        matches!(outcome, tandem_app::JobOutcome::Completed { .. }),
        "{outcome:?}"
    );
    let reply_event = next_json(&mut feed, Duration::from_secs(5)).await.unwrap();
    assert_eq!(reply_event["author_is_agent"], true);
    assert!(reply_event["id"].as_i64() > first["id"].as_i64());

    let messages = stack
        .send(get(&format!("/api/chat/sessions/{session_id}/messages")).cookie(&member.cookie))
        .await
        .json();
    assert_eq!(messages.as_array().unwrap().len(), 2);
    assert_eq!(messages[1]["author_kind"], "agent");
    assert_eq!(messages[1]["in_reply_to"], messages[0]["id"]);

    // Another user never sees these events.
    assert!(
        next_json(&mut admin_feed, Duration::from_millis(1500))
            .await
            .is_none(),
        "the admin's feed carries nothing about the member's chat"
    );

    // Replay from the start returns both events after the ready frame.
    let mut replay = open_feed(
        &stack,
        Some(&member.cookie),
        Some(&stack.origin),
        "?after=0",
    )
    .await
    .unwrap();
    assert_eq!(
        next_json(&mut replay, Duration::from_secs(3))
            .await
            .unwrap()["type"],
        "ready"
    );
    let replayed_first = next_json(&mut replay, Duration::from_secs(3))
        .await
        .unwrap();
    let replayed_second = next_json(&mut replay, Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(replayed_first["id"], first["id"]);
    assert_eq!(replayed_second["id"], reply_event["id"]);

    // The per-user connection limit is ten.
    let mut held = vec![feed, replay];
    for _ in 0..8 {
        held.push(
            open_feed(&stack, Some(&member.cookie), Some(&stack.origin), "")
                .await
                .unwrap(),
        );
    }
    let eleventh = open_feed(&stack, Some(&member.cookie), Some(&stack.origin), "").await;
    assert_eq!(handshake_status(&eleventh), Some(429));
    drop(held.pop());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        open_feed(&stack, Some(&member.cookie), Some(&stack.origin), "")
            .await
            .is_ok()
    );

    // The audit log has the trail, newest first, with working paging.
    let page = stack
        .send(get("/api/admin/audit?limit=2").cookie(&admin.cookie))
        .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text);
    let body = page.json();
    assert_eq!(body["entries"].as_array().unwrap().len(), 2);
    let after_time = body["next_after_time"].as_str().unwrap().to_owned();
    let after_id = body["next_after_id"].as_str().unwrap().to_owned();
    let next = stack
        .send(
            get(&format!(
                "/api/admin/audit?limit=100&after_time={after_time}&after_id={after_id}"
            ))
            .cookie(&admin.cookie),
        )
        .await
        .json();
    let types: Vec<&str> = next["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["event_type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"auth.setup"), "{types:?}");
    let filtered = stack
        .send(get("/api/admin/audit?event_type=auth.setup").cookie(&admin.cookie))
        .await
        .json();
    assert_eq!(filtered["entries"].as_array().unwrap().len(), 1);
    let bad_filter = stack
        .send(get("/api/admin/audit?event_type=nope").cookie(&admin.cookie))
        .await;
    assert_eq!(bad_filter.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Revoking a pending invitation works through the API.
    let pending = stack
        .send(
            stack
                .signed_in(post("/api/admin/invitations"), &admin)
                .json(&json!({"email": "bo@example.test", "role": "member"})),
        )
        .await
        .json();
    let id = pending["invitation"]["id"].as_str().unwrap().to_owned();
    let revoke = stack
        .send(stack.signed_in(delete(&format!("/api/admin/invitations/{id}")), &admin))
        .await;
    assert_eq!(revoke.status, StatusCode::NO_CONTENT, "{}", revoke.text);
    let listed = stack
        .send(get("/api/admin/invitations").cookie(&admin.cookie))
        .await
        .json();
    let revoked = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|invitation| invitation["id"] == id.as_str())
        .unwrap();
    assert_eq!(revoked["status"], "revoked");

    // Shutdown sends the sockets a close frame.
    let mut last = held.pop().unwrap();
    stack.state.readiness.begin_shutdown();
    stack.shutdown.trigger();
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match last.next().await {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return true,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert_eq!(closed, Ok(true));
    assert_eq!(
        stack.send(get("/readyz")).await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn start_up_refuses_a_wrong_vault_key_and_a_stale_schema() {
    let db = pg::TestDb::new().await;
    sqlx::query(
        "INSERT INTO tandem.instance_settings (key, value, updated_at)
         VALUES ('internal.hash_profile', '19456:2:1', now()) ON CONFLICT DO NOTHING",
    )
    .execute(&db.superuser)
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let environment = db.environment(dir.path());
    // The first start stores the canary under the dev key.
    Services::start(Config::load(&environment).unwrap(), Process::Api)
        .await
        .unwrap();

    // A different key must not start.
    let other_key = dir.path().join("other.key");
    std::fs::write(
        &other_key,
        "k9 current AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=\n",
    )
    .unwrap();
    let mut wrong = environment.clone();
    wrong.insert(
        "TANDEM_MASTER_KEY_FILE".to_owned(),
        other_key.display().to_string(),
    );
    let error = Services::start(Config::load(&wrong).unwrap(), Process::Api)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("wrong or missing key"), "{error}");
    assert!(!error.contains("AgIC"), "no key material in the message");

    // A schema one step behind must not start either.
    tandem_store_postgres::migrate_down_one(&db.migrate)
        .await
        .unwrap();
    let error = Services::start(Config::load(&environment).unwrap(), Process::Api)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.to_lowercase().contains("schema"), "{error}");
}

// ---- U2 instance administration ------------------------------------------------------------

#[derive(Clone, Default)]
struct LogCapture(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogCapture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl LogCapture {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// An OpenAI-compatible server on loopback that remembers the Authorization headers it saw.
async fn mock_openai() -> (std::net::SocketAddr, Arc<std::sync::Mutex<Vec<String>>>) {
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::routing::{get as route_get, post as route_post};
    let seen: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    async fn models(
        State(seen): State<Arc<std::sync::Mutex<Vec<String>>>>,
        headers: HeaderMap,
    ) -> String {
        if let Some(value) = headers.get("authorization") {
            seen.lock()
                .unwrap()
                .push(value.to_str().unwrap().to_owned());
        }
        r#"{"data":[{"id":"llama3","context_length":32768}]}"#.to_owned()
    }
    async fn chat(body: String) -> String {
        if body.contains("\"tools\"") {
            r#"{"choices":[{"message":{"tool_calls":[{"function":{"name":"echo","arguments":"{}"}}]}}]}"#
        } else {
            r#"{"choices":[{"message":{"content":"ok"}}]}"#
        }
        .to_owned()
    }
    let router = Router::new()
        .route("/v1/models", route_get(models))
        .route("/v1/chat/completions", route_post(chat))
        .with_state(Arc::clone(&seen));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, seen)
}

#[tokio::test]
async fn a_provider_test_runs_end_to_end_over_postgres_with_the_real_adapter_and_leaks_nothing() {
    const KEY: &str = "sk-e2e-must-never-be-stored-or-logged";
    let logs = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer({
            let logs = logs.clone();
            move || logs.clone()
        })
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let stack = Stack::start().await;
    let setup = stack
        .send(post("/api/setup").origin_of(&stack.origin).json(&json!({
            "display_name": "Ada Admin", "email": "ada@example.test", "password": PASSWORD
        })))
        .await;
    assert_eq!(setup.status, StatusCode::OK, "{}", setup.text);
    let admin = stack.identify(&setup.session_cookie().unwrap()).await;
    let enrol = stack
        .send(stack.signed_in(post("/api/auth/mfa/enrol"), &admin))
        .await;
    let secret = enrol.json()["secret_base32"].as_str().unwrap().to_owned();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let confirm = stack
        .send(
            stack
                .signed_in(post("/api/auth/mfa/confirm"), &admin)
                .json(&json!({"code": totp(&secret, now)})),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::OK, "{}", confirm.text);

    let (address, seen) = mock_openai().await;
    let created = stack
        .send(
            stack
                .signed_in(post("/api/admin/providers"), &admin)
                .json(&json!({
                    "name": "Mock", "base_url": format!("http://{address}/v1"), "api_key": KEY,
                    "models": [{"name": "llama3"}]
                })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    let id = created.json()["id"].as_str().unwrap().to_owned();
    assert!(!created.text.contains(KEY));

    // The key is sealed at rest: the stored bytes are not the text.
    let stored: Vec<u8> = sqlx::query_scalar("SELECT api_key_sealed FROM tandem.providers")
        .fetch_one(&stack._db.superuser)
        .await
        .unwrap();
    assert!(
        !stored
            .windows(KEY.len())
            .any(|window| window == KEY.as_bytes())
    );

    let started = stack
        .send(stack.signed_in(post(&format!("/api/admin/providers/{id}/test")), &admin))
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED, "{}", started.text);
    let run_id = started.json()["id"].as_str().unwrap().to_owned();
    let duplicate = stack
        .send(stack.signed_in(post(&format!("/api/admin/providers/{id}/test")), &admin))
        .await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
    assert_eq!(duplicate.json()["error"]["run_id"], run_id);

    let outcome = stack
        .services
        .app
        .process_next_job(time::Duration::seconds(60))
        .await
        .unwrap();
    assert!(
        matches!(
            outcome,
            tandem_app::JobOutcome::Completed {
                kind: "config_test"
            }
        ),
        "{outcome:?}"
    );

    let run = stack
        .send(stack.signed_in(get(&format!("/api/admin/test-runs/{run_id}")), &admin))
        .await
        .json();
    assert_eq!(run["status"], "succeeded", "{run}");
    let statuses: Vec<&str> = run["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| result["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["ok", "ok"], "the provider and its model");
    let provider = stack
        .send(stack.signed_in(get(&format!("/api/admin/providers/{id}")), &admin))
        .await
        .json();
    assert_eq!(provider["models"][0]["detected_tool_calling"], "supported");
    assert_eq!(provider["models"][0]["detected_context_tokens"], 32768);
    assert!(
        provider["models"][0]["warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        [format!("Bearer {KEY}")],
        "the key reached the provider"
    );

    let audit = stack
        .send(stack.signed_in(get("/api/admin/audit?limit=100"), &admin))
        .await;
    let audit_json = audit.json();
    let types: Vec<&str> = audit_json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["event_type"].as_str().unwrap())
        .collect();
    assert!(
        types.contains(&"config.provider.created") && types.contains(&"config.test.run"),
        "{types:?}"
    );
    assert!(!audit.text.contains(KEY));

    let captured = logs.text();
    assert!(
        captured.contains("outbound call"),
        "the call was made and logged: {captured}"
    );
    assert!(!captured.contains(KEY), "BR-20.4: a key reached a log line");
}
