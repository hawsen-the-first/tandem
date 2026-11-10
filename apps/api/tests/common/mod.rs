//! In-memory harness: the real router and pipeline over the application's in-memory fakes.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use serde_json::Value;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tandem_api::server::{StateInputs, build};
use tandem_api::state::AppState;
use tandem_app::testing::TestApp;
use tandem_config::Config;
use tandem_runtime::Shutdown;
use tandem_telemetry::{MetricsHandle, MetricsRegistry, Readiness, ReadinessProbe};
use tempfile::TempDir;
use tower::ServiceExt;

pub const ORIGIN: &str = "https://tandem.test";
pub const PASSWORD: &str = "Correct-Horse-7";

static METRICS: OnceLock<MetricsHandle> = OnceLock::new();

fn metrics() -> MetricsHandle {
    METRICS
        .get_or_init(|| {
            MetricsRegistry::new("test")
                .unwrap()
                .install()
                .expect("one global recorder per test binary")
        })
        .clone()
}

struct FlagProbe(Arc<AtomicBool>);

#[async_trait::async_trait]
impl ReadinessProbe for FlagProbe {
    fn name(&self) -> &'static str {
        "database"
    }
    async fn check(&self) -> Result<(), String> {
        if self.0.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("secret-detail-that-must-not-leak".to_owned())
        }
    }
}

#[derive(Default, Clone)]
pub struct Options {
    pub trusted_proxies: Option<&'static str>,
    pub metrics_token: Option<&'static str>,
    pub web_dir: Option<std::path::PathBuf>,
}

pub struct Harness {
    pub router: Router,
    pub state: AppState,
    pub t: TestApp,
    pub database_ok: Arc<AtomicBool>,
    _dir: TempDir,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub text: String,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text)
            .unwrap_or_else(|error| panic!("not JSON ({error}): {}", self.text))
    }

    pub fn error_code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    }

    pub fn header(&self, name: &str) -> String {
        self.headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned()
    }

    pub fn session_cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|line| {
                let (pair, _) = line.split_once(';')?;
                let (name, value) = pair.split_once('=')?;
                (name == "__Host-tandem_session" && !value.is_empty()).then(|| pair.to_owned())
            })
    }
}

pub struct Call {
    method: Method,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    peer: SocketAddr,
}

pub fn get(path: &str) -> Call {
    Call::new(Method::GET, path)
}
pub fn post(path: &str) -> Call {
    Call::new(Method::POST, path)
}
pub fn put(path: &str) -> Call {
    Call::new(Method::PUT, path)
}
pub fn delete(path: &str) -> Call {
    Call::new(Method::DELETE, path)
}

impl Call {
    fn new(method: Method, path: &str) -> Self {
        Self {
            method,
            path: path.to_owned(),
            headers: Vec::new(),
            body: None,
            peer: "198.51.100.7:4000".parse().unwrap(),
        }
    }
    pub fn json(mut self, body: &Value) -> Self {
        self.headers
            .push(("content-type".to_owned(), "application/json".to_owned()));
        self.body = Some(body.to_string());
        self
    }
    pub fn raw_body(mut self, content_type: &str, body: &str) -> Self {
        self.headers
            .push(("content-type".to_owned(), content_type.to_owned()));
        self.body = Some(body.to_owned());
        self
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
    pub fn origin(self) -> Self {
        self.header("origin", ORIGIN)
    }
    pub fn cookie(self, cookie: &str) -> Self {
        self.header("cookie", cookie)
    }
    pub fn csrf(self, token: &str) -> Self {
        self.header("x-csrf-token", token)
    }
    pub fn peer(mut self, address: &str) -> Self {
        self.peer = address.parse().unwrap();
        self
    }
    /// A signed-in caller: cookie, CSRF token and Origin.
    pub fn signed_in(self, user: &User) -> Self {
        self.cookie(&user.cookie).csrf(&user.csrf).origin()
    }
}

#[derive(Clone)]
pub struct User {
    pub cookie: String,
    pub csrf: String,
    pub id: String,
}

impl Harness {
    pub fn new() -> Self {
        Self::with(Options::default())
    }

    pub fn with(options: Options) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ca = dir.path().join("ca.crt");
        let key = dir.path().join("master.key");
        std::fs::write(&ca, "ca").unwrap();
        std::fs::write(&key, "k1 current AAAA").unwrap();
        let mut environment: BTreeMap<String, String> = [
            ("TANDEM_PUBLIC_URL", ORIGIN),
            (
                "TANDEM_DATABASE_URL",
                "postgres://app:unused@db/tandem?sslmode=verify-full",
            ),
            ("TANDEM_DATABASE_CA_FILE", ca.to_str().unwrap()),
            ("TANDEM_MASTER_KEY_FILE", key.to_str().unwrap()),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        if let Some(proxies) = options.trusted_proxies {
            environment.insert("TANDEM_TRUSTED_PROXIES".to_owned(), proxies.to_owned());
        }
        if let Some(token) = options.metrics_token {
            environment.insert("TANDEM_METRICS_TOKEN".to_owned(), token.to_owned());
        }
        let config = Arc::new(Config::load(&environment).unwrap());
        let t = TestApp::new();
        let database_ok = Arc::new(AtomicBool::new(true));
        let readiness = Readiness::new(
            vec![Arc::new(FlagProbe(database_ok.clone()))],
            Duration::from_secs(1),
        );
        let (router, state) = build(StateInputs {
            app: t.app.clone(),
            config,
            readiness,
            metrics: metrics(),
            shutdown: Shutdown::new(),
            web_dir: options.web_dir.clone(),
        })
        .unwrap();
        Self {
            router,
            state,
            t,
            database_ok,
            _dir: dir,
        }
    }

    pub async fn send(&self, call: Call) -> Reply {
        send_to(&self.router, call).await
    }

    /// The CSRF token and ID of a signed-in user.
    pub async fn identify(&self, cookie: &str) -> User {
        let reply = self.send(get("/api/auth/me").cookie(cookie)).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
        let body = reply.json();
        User {
            cookie: cookie.to_owned(),
            csrf: body["csrf_token"].as_str().unwrap().to_owned(),
            id: body["user"]["id"].as_str().unwrap().to_owned(),
        }
    }

    /// Run first-run setup and finish MFA enrolment: a full-scope admin.
    pub async fn setup_admin(&self, email: &str) -> User {
        let reply = self
            .send(post("/api/setup").origin().json(&serde_json::json!({
                "display_name": "Ada Admin", "email": email, "password": PASSWORD
            })))
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
        let cookie = reply.session_cookie().expect("session cookie");
        let user = self.identify(&cookie).await;
        let enrol = self
            .send(post("/api/auth/mfa/enrol").signed_in(&user))
            .await;
        assert_eq!(enrol.status, StatusCode::OK, "{}", enrol.text);
        let code = self.t.current_code(user.id.parse().unwrap());
        let confirm = self
            .send(
                post("/api/auth/mfa/confirm")
                    .signed_in(&user)
                    .json(&serde_json::json!({ "code": code })),
            )
            .await;
        assert_eq!(confirm.status, StatusCode::OK, "{}", confirm.text);
        user
    }

    /// Invite and accept: a member.
    pub async fn add_member(&self, admin: &User, email: &str) -> User {
        let invite = self
            .send(
                post("/api/admin/invitations")
                    .signed_in(admin)
                    .json(&serde_json::json!({ "email": email, "role": "member" })),
            )
            .await;
        assert_eq!(invite.status, StatusCode::CREATED, "{}", invite.text);
        let link = invite.json()["link"].as_str().unwrap().to_owned();
        let token = link.split("token=").nth(1).unwrap().to_owned();
        let accept = self
            .send(
                post("/api/invitations/accept")
                    .origin()
                    .json(&serde_json::json!({
                        "token": token, "display_name": "Mia Member", "password": PASSWORD
                    })),
            )
            .await;
        assert_eq!(accept.status, StatusCode::OK, "{}", accept.text);
        self.identify(&accept.session_cookie().unwrap()).await
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Reply {
        self.send(post("/api/auth/sign-in").origin().json(&serde_json::json!({
            "email": email, "password": password
        })))
        .await
    }
}

/// Send one request through `router` as if it arrived over the network.
pub async fn send_to(router: &Router, call: Call) -> Reply {
    let mut builder = Request::builder().method(call.method).uri(call.path);
    for (name, value) in call.headers {
        builder = builder.header(name, value);
    }
    let mut request = builder
        .body(Body::from(call.body.unwrap_or_default()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(call.peer));
    let response = router.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, 4_000_000).await.unwrap();
    Reply {
        status: parts.status,
        headers: parts.headers,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

impl Call {
    /// The same-origin header for an arbitrary public origin.
    pub fn origin_of(self, origin: &str) -> Self {
        self.header("origin", origin)
    }
}
