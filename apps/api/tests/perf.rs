//! Performance smoke test (PF-1 to PF-3, PF-6): a small seeded dataset, a real PostgreSQL, the real
//! router and a real WebSocket, with the budgets from the requirements. It is `#[ignore]`d in the
//! ordinary test run and executed by the `perf` CI job in release mode:
//!
//! ```text
//! cargo test -p tandem-api --test perf --release -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

mod common;
#[path = "postgres_common/mod.rs"]
mod pg;

use axum::Router;
use axum::http::StatusCode;
use common::{Call, Reply, get, post, send_to};
use futures_util::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha1::Sha1;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tandem_api::routes::ws::spawn_listener;
use tandem_api::server::{StateInputs, build, serve};
use tandem_config::Config;
use tandem_runtime::{DatabaseProbe, Process, Services, Shutdown};
use tandem_telemetry::{MetricsHandle, MetricsRegistry, Readiness, ReadinessProbe};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const PASSWORD: &str = "Correct-Horse-7";

/// PF-1: typical calls, 95th percentile.
const READ_BUDGET: Duration = Duration::from_millis(200);
/// PF-2: sign-in including hashing.
const SIGN_IN_BUDGET: Duration = Duration::from_secs(1);
/// PF-3: post a message and see the agent's reply on the socket.
const ROUND_TRIP_BUDGET: Duration = Duration::from_millis(500);

fn metrics() -> MetricsHandle {
    static HANDLE: OnceLock<MetricsHandle> = OnceLock::new();
    HANDLE
        .get_or_init(|| MetricsRegistry::new("perf").unwrap().install().unwrap())
        .clone()
}

fn percentile(samples: &mut [Duration], fraction: f64) -> Duration {
    samples.sort();
    let index = ((samples.len() as f64 * fraction).ceil() as usize).clamp(1, samples.len()) - 1;
    samples[index]
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

struct Person {
    cookie: String,
    csrf: String,
}

async fn identify(router: &Router, cookie: &str) -> Person {
    let body = send_to(router, get("/api/auth/me").cookie(cookie))
        .await
        .json();
    Person {
        cookie: cookie.to_owned(),
        csrf: body["csrf_token"].as_str().unwrap().to_owned(),
    }
}

fn signed_in(call: Call, person: &Person, origin: &str) -> Call {
    call.cookie(&person.cookie)
        .csrf(&person.csrf)
        .origin_of(origin)
}

#[tokio::test]
#[ignore = "run by the perf CI job in release mode"]
async fn the_service_meets_its_latency_budgets() {
    let db = pg::TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    // No seeded hashing profile: the first start benchmarks this host, as production does.
    let config = Config::load(&db.environment(dir.path())).unwrap();
    let origin = config.public_origin();
    let services = Services::start(config, Process::Api).await.unwrap();
    eprintln!(
        "hash profile: {} MiB, {} iterations",
        services.hash_profile.memory_mib(),
        services.hash_profile.iterations
    );
    let shutdown = Shutdown::new();
    let pool = services.store.pool().clone();
    let probes: Vec<Arc<dyn ReadinessProbe>> = vec![Arc::new(DatabaseProbe::new(pool.clone()))];
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

    // Seed: an admin with MFA and a few members, one with a chat session.
    let setup = send_to(
        &router,
        post("/api/setup").origin_of(&origin).json(&json!({
            "display_name": "Ada Admin", "email": "ada@example.test", "password": PASSWORD
        })),
    )
    .await;
    assert_eq!(setup.status, StatusCode::OK, "{}", setup.text);
    let admin = identify(&router, &setup.session_cookie().unwrap()).await;
    let enrol = send_to(
        &router,
        signed_in(post("/api/auth/mfa/enrol"), &admin, &origin),
    )
    .await;
    let secret = enrol.json()["secret_base32"].as_str().unwrap().to_owned();
    let confirm = send_to(
        &router,
        signed_in(post("/api/auth/mfa/confirm"), &admin, &origin).json(
            &json!({"code": totp(&secret, time::OffsetDateTime::now_utc().unix_timestamp())}),
        ),
    )
    .await;
    assert_eq!(confirm.status, StatusCode::OK, "{}", confirm.text);
    let mut members = Vec::new();
    for index in 0..5 {
        let email = format!("member{index}@example.test");
        let invite = send_to(
            &router,
            signed_in(post("/api/admin/invitations"), &admin, &origin)
                .json(&json!({"email": email, "role": "member"})),
        )
        .await
        .json();
        let token = invite["link"]
            .as_str()
            .unwrap()
            .split("token=")
            .nth(1)
            .unwrap()
            .to_owned();
        let accepted = send_to(
            &router,
            post("/api/invitations/accept").origin_of(&origin).json(
                &json!({"token": token, "display_name": format!("Member {index}"), "password": PASSWORD}),
            ),
        )
        .await;
        assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.text);
        members.push((
            email,
            identify(&router, &accepted.session_cookie().unwrap()).await,
        ));
    }
    let owner = &members[0].1;
    let chat = send_to(
        &router,
        signed_in(post("/api/chat/sessions"), owner, &origin).json(&json!({"title": "Perf"})),
    )
    .await
    .json();
    let chat_id = chat["id"].as_str().unwrap().to_owned();

    // PF-2: sign-in latency, sequential, five members four times each.
    let mut sign_ins = Vec::new();
    for round in 0..4 {
        for (email, _) in &members {
            let started = Instant::now();
            let reply: Reply = send_to(
                &router,
                post("/api/auth/sign-in")
                    .origin_of(&origin)
                    .peer(&format!("198.51.100.{}:1", 10 + round))
                    .json(&json!({"email": email, "password": PASSWORD})),
            )
            .await;
            sign_ins.push(started.elapsed());
            assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
        }
    }
    let sign_in_p95 = percentile(&mut sign_ins, 0.95);
    eprintln!("PF-2 sign-in p95: {sign_in_p95:?} (budget {SIGN_IN_BUDGET:?})");
    assert!(sign_in_p95 < SIGN_IN_BUDGET, "sign-in p95 {sign_in_p95:?}");

    // PF-1: typical reads with eight callers at once.
    let mut tasks = JoinSet::new();
    for worker in 0..8 {
        let router = router.clone();
        let cookie = owner.cookie.clone();
        let path = if worker % 2 == 0 {
            "/api/auth/me".to_owned()
        } else {
            "/api/chat/sessions".to_owned()
        };
        let peer = format!("198.51.100.{}:1", 100 + worker);
        tasks.spawn(async move {
            let mut samples = Vec::new();
            for _ in 0..40 {
                let started = Instant::now();
                let reply = send_to(&router, get(&path).cookie(&cookie).peer(&peer)).await;
                samples.push(started.elapsed());
                assert_eq!(reply.status, StatusCode::OK);
            }
            samples
        });
    }
    let mut reads = Vec::new();
    while let Some(done) = tasks.join_next().await {
        reads.extend(done.unwrap());
    }
    let read_p95 = percentile(&mut reads, 0.95);
    eprintln!("PF-1 read p95: {read_p95:?} (budget {READ_BUDGET:?})");
    assert!(read_p95 < READ_BUDGET, "read p95 {read_p95:?}");

    // PF-3: post a message and wait for the agent's reply on the socket. A task stands in for the
    // worker's relay and job loops, polling at the interval the worker uses.
    let driver_app = services.app.clone();
    let driver_stop = shutdown.subscribe();
    tokio::spawn(async move {
        while !*driver_stop.borrow() {
            let _ = driver_app.relay_outbox_batch(100).await;
            while matches!(
                driver_app
                    .process_next_job(time::Duration::seconds(60))
                    .await,
                Ok(tandem_app::JobOutcome::Completed { .. })
            ) {}
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    let url = format!("ws://{address}/api/events");
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("cookie", owner.cookie.parse().unwrap());
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let _ready = socket.next().await;
    let mut round_trips = Vec::new();
    for index in 0..20 {
        let started = Instant::now();
        let posted = send_to(
            &router,
            signed_in(
                post(&format!("/api/chat/sessions/{chat_id}/messages")),
                owner,
                &origin,
            )
            .json(&json!({"body": format!("round trip {index}")})),
        )
        .await;
        assert_eq!(posted.status, StatusCode::CREATED);
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await
                .expect("the agent replies within five seconds")
                .unwrap()
                .unwrap();
            if let Message::Text(text) = frame
                && serde_json::from_str::<Value>(&text).unwrap()["author_is_agent"] == true
            {
                break;
            }
        }
        round_trips.push(started.elapsed());
    }
    let round_trip_p95 = percentile(&mut round_trips, 0.95);
    eprintln!("PF-3 round trip p95: {round_trip_p95:?} (budget {ROUND_TRIP_BUDGET:?})");
    assert!(
        round_trip_p95 < ROUND_TRIP_BUDGET,
        "round trip p95 {round_trip_p95:?}"
    );
    shutdown.trigger();
}
