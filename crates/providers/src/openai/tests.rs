use super::*;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tandem_core::config::ModelName;
use tandem_core::ids::ProviderModelId;
use tandem_core::outbound_url::OutboundUrl;
use tandem_outbound::{Limits, SystemLookup};
use tandem_support::Secret;
use tokio::net::TcpListener;

type ChatReply = Arc<dyn Fn(&Value) -> (StatusCode, String) + Send + Sync>;

#[derive(Clone)]
struct Server {
    listing: (StatusCode, String),
    chat: ChatReply,
    authorisations: Arc<Mutex<Vec<String>>>,
    chat_calls: Arc<AtomicUsize>,
    running: Arc<AtomicUsize>,
    most_running: Arc<AtomicUsize>,
}

impl Server {
    fn new(listing: (StatusCode, String), chat: ChatReply) -> Self {
        Self {
            listing,
            chat,
            authorisations: Arc::default(),
            chat_calls: Arc::default(),
            running: Arc::default(),
            most_running: Arc::default(),
        }
    }

    async fn start(&self) -> SocketAddr {
        async fn listing(State(server): State<Server>, headers: HeaderMap) -> (StatusCode, String) {
            if let Some(value) = headers.get("authorization") {
                server
                    .authorisations
                    .lock()
                    .unwrap()
                    .push(value.to_str().unwrap().to_owned());
            }
            server.listing.clone()
        }
        async fn chat(State(server): State<Server>, body: String) -> (StatusCode, String) {
            server.chat_calls.fetch_add(1, Ordering::SeqCst);
            let now = server.running.fetch_add(1, Ordering::SeqCst) + 1;
            server.most_running.fetch_max(now, Ordering::SeqCst);
            let request: Value = serde_json::from_str(&body).unwrap();
            let answer = (server.chat)(&request);
            server.running.fetch_sub(1, Ordering::SeqCst);
            answer
        }
        let router = Router::new()
            .route("/v1/models", get(listing))
            .route("/v1/chat/completions", post(chat))
            .with_state(self.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        address
    }
}

#[derive(Default)]
struct Collected(Mutex<Vec<ProbeResult>>);

#[async_trait]
impl ResultSink for Collected {
    async fn record(&self, result: ProbeResult) {
        self.0.lock().unwrap().push(result);
    }
}

impl Collected {
    fn all(&self) -> Vec<ProbeResult> {
        self.0.lock().unwrap().clone()
    }

    fn of(&self, subject: ProbeSubject) -> ProbeResult {
        self.all()
            .into_iter()
            .rev()
            .find(|result| result.subject == subject)
            .unwrap_or_else(|| panic!("no result for {subject:?}"))
    }
}

fn model(number: u8, name: &str) -> ModelToProbe {
    ModelToProbe {
        id: format!("00000000-0000-0000-0000-0000000000{number:02}")
            .parse::<ProviderModelId>()
            .unwrap(),
        name: ModelName::parse(name).unwrap(),
    }
}

fn connection(address: SocketAddr, key: Option<&str>) -> ProviderConnection {
    ProviderConnection {
        base_url: OutboundUrl::parse(&format!("http://{address}/v1")).unwrap(),
        api_key: key.map(Secret::new),
    }
}

fn probe() -> OpenAiCompatibleProbe {
    OpenAiCompatibleProbe::new(OutboundClient::new().unwrap())
}

fn ok(body: &str) -> (StatusCode, String) {
    (StatusCode::OK, body.to_owned())
}

const PLAIN_ANSWER: &str = r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
const TOOL_ANSWER: &str = r#"{"choices":[{"message":{"role":"assistant","content":null,
    "tool_calls":[{"id":"1","type":"function","function":{"name":"echo","arguments":"{\"text\":\"ok\"}"}}]}}]}"#;

fn chat_with_tools() -> ChatReply {
    Arc::new(|request| {
        if request.get("tools").is_some() {
            ok(TOOL_ANSWER)
        } else {
            ok(PLAIN_ANSWER)
        }
    })
}

fn listing_of(entries: &str) -> (StatusCode, String) {
    ok(&format!(r#"{{"object":"list","data":[{entries}]}}"#))
}

#[tokio::test]
async fn a_capable_model_is_detected_with_tool_calling_and_its_context_length() {
    let server = Server::new(
        listing_of(r#"{"id":"big","context_length":32768},{"id":"other","max_model_len":4096}"#),
        chat_with_tools(),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "big");

    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;

    assert_eq!(sink.of(ProbeSubject::Provider).status, TestStatus::Ok);
    let result = sink.of(ProbeSubject::Model(target.id));
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.details.tool_calling, Some(ToolCalling::Supported));
    assert_eq!(result.details.context_tokens, Some(32768));
    assert_eq!(
        server.chat_calls.load(Ordering::SeqCst),
        2,
        "chat then tool probe"
    );
}

#[tokio::test]
async fn each_context_field_a_provider_may_use_is_understood() {
    for field in ["context_length", "max_model_len", "context_window"] {
        let server = Server::new(
            listing_of(&format!(r#"{{"id":"m","{field}":16000}}"#)),
            chat_with_tools(),
        );
        let address = server.start().await;
        let sink = Collected::default();
        let target = model(1, "m");
        probe()
            .probe(
                &connection(address, None),
                std::slice::from_ref(&target),
                &sink,
            )
            .await;
        assert_eq!(
            sink.of(ProbeSubject::Model(target.id))
                .details
                .context_tokens,
            Some(16000),
            "{field}"
        );
    }
}

#[tokio::test]
async fn the_key_is_sent_as_a_bearer_token_and_appears_in_no_result() {
    let key = "sk-test-not-a-real-key";
    let server = Server::new(listing_of(r#"{"id":"m"}"#), chat_with_tools());
    let address = server.start().await;
    let sink = Collected::default();
    probe()
        .probe(&connection(address, Some(key)), &[model(1, "m")], &sink)
        .await;
    assert_eq!(
        server.authorisations.lock().unwrap().as_slice(),
        [format!("Bearer {key}")]
    );
    assert!(!format!("{:?}", sink.all()).contains(key));
}

#[tokio::test]
async fn a_model_that_answers_in_plain_text_has_no_tool_calling() {
    let server = Server::new(listing_of(r#"{"id":"m"}"#), Arc::new(|_| ok(PLAIN_ANSWER)));
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "m");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    let result = sink.of(ProbeSubject::Model(target.id));
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.details.tool_calling, Some(ToolCalling::Unsupported));
}

#[tokio::test]
async fn a_provider_that_rejects_tools_means_unsupported() {
    let server = Server::new(
        listing_of(r#"{"id":"m"}"#),
        Arc::new(|request| {
            if request.get("tools").is_some() {
                (
                    StatusCode::BAD_REQUEST,
                    r#"{"error":{"message":"no tools"}}"#.into(),
                )
            } else {
                ok(PLAIN_ANSWER)
            }
        }),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "m");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).details.tool_calling,
        Some(ToolCalling::Unsupported)
    );
}

#[tokio::test]
async fn an_inconclusive_tool_probe_is_unknown() {
    let server = Server::new(
        listing_of(r#"{"id":"m"}"#),
        Arc::new(|request| {
            if request.get("tools").is_some() {
                (StatusCode::INTERNAL_SERVER_ERROR, "boom".into())
            } else {
                ok(PLAIN_ANSWER)
            }
        }),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "m");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).details.tool_calling,
        Some(ToolCalling::Unknown)
    );
}

#[tokio::test]
async fn a_provider_without_a_model_listing_is_still_tested_through_its_models() {
    let server = Server::new((StatusCode::NOT_FOUND, String::new()), chat_with_tools());
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "m");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(sink.of(ProbeSubject::Provider).status, TestStatus::Ok);
    let result = sink.of(ProbeSubject::Model(target.id));
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(
        result.details.context_tokens, None,
        "unknown until declared (A6)"
    );
}

#[tokio::test]
async fn a_refused_key_stops_the_test_before_any_model_is_called() {
    let server = Server::new(
        (
            StatusCode::UNAUTHORIZED,
            r#"{"error":"SECRET-REMOTE-TEXT"}"#.into(),
        ),
        chat_with_tools(),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let models = [model(1, "a"), model(2, "b")];
    probe()
        .probe(&connection(address, Some("wrong")), &models, &sink)
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Provider).status,
        TestStatus::Unauthorised
    );
    for target in &models {
        assert_eq!(
            sink.of(ProbeSubject::Model(target.id)).status,
            TestStatus::Unauthorised
        );
    }
    assert_eq!(server.chat_calls.load(Ordering::SeqCst), 0);
    assert!(!format!("{:?}", sink.all()).contains("SECRET-REMOTE-TEXT"));
}

#[tokio::test]
async fn an_unknown_model_is_reported_and_the_provider_still_counts_as_working() {
    let server = Server::new(
        (StatusCode::NOT_FOUND, String::new()),
        Arc::new(|_| (StatusCode::NOT_FOUND, r#"{"error":"nope"}"#.into())),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "typo");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).status,
        TestStatus::UnknownModel
    );
    assert_eq!(sink.of(ProbeSubject::Provider).status, TestStatus::Ok);
}

#[tokio::test]
async fn a_named_model_not_found_error_is_an_unknown_model_even_as_a_400() {
    let server = Server::new(
        listing_of(r#"{"id":"typo"}"#),
        Arc::new(|_| {
            (
                StatusCode::BAD_REQUEST,
                r#"{"error":{"code":"model_not_found","message":"x"}}"#.into(),
            )
        }),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let target = model(1, "typo");
    probe()
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).status,
        TestStatus::UnknownModel
    );
}

#[tokio::test]
async fn an_answer_that_is_not_a_chat_completion_is_a_bad_response() {
    for body in ["<html>hello</html>", "{}", r#"{"choices":[]}"#] {
        let server = Server::new(
            (StatusCode::NOT_FOUND, String::new()),
            Arc::new(move |_| ok(body)),
        );
        let address = server.start().await;
        let sink = Collected::default();
        let target = model(1, "m");
        probe()
            .probe(
                &connection(address, None),
                std::slice::from_ref(&target),
                &sink,
            )
            .await;
        assert_eq!(
            sink.of(ProbeSubject::Model(target.id)).status,
            TestStatus::BadResponse,
            "{body}"
        );
        assert_eq!(
            sink.of(ProbeSubject::Provider).status,
            TestStatus::BadResponse,
            "{body}"
        );
    }
}

#[tokio::test]
async fn an_oversized_answer_is_a_bad_response() {
    let server = Server::new(
        (StatusCode::NOT_FOUND, String::new()),
        Arc::new(|_| ok(&"x".repeat(5000))),
    );
    let address = server.start().await;
    let limits = Limits {
        max_response_bytes: 1000,
        ..Limits::default()
    };
    let client = OutboundClient::with(limits, Arc::new(SystemLookup)).unwrap();
    let sink = Collected::default();
    let target = model(1, "m");
    OpenAiCompatibleProbe::new(client)
        .probe(
            &connection(address, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).status,
        TestStatus::BadResponse
    );
}

#[tokio::test]
async fn a_provider_that_cannot_be_reached_fails_the_provider_and_every_model() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let sink = Collected::default();
    let target = model(1, "m");
    probe()
        .probe(
            &connection(closed, None),
            std::slice::from_ref(&target),
            &sink,
        )
        .await;
    assert_eq!(
        sink.of(ProbeSubject::Provider).status,
        TestStatus::Unreachable
    );
    assert_eq!(
        sink.of(ProbeSubject::Model(target.id)).status,
        TestStatus::Unreachable
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_model_times_out_when_the_budget_runs_out_but_finished_ones_are_kept() {
    let server = Server::new(
        listing_of(r#"{"id":"fast"},{"id":"slow"}"#),
        Arc::new(|request| {
            if request["model"] == "slow" {
                std::thread::sleep(Duration::from_millis(1500));
            }
            ok(PLAIN_ANSWER)
        }),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let (fast, slow) = (model(1, "fast"), model(2, "slow"));
    probe()
        .with_budget(Duration::from_millis(500))
        .probe(
            &connection(address, None),
            &[fast.clone(), slow.clone()],
            &sink,
        )
        .await;
    assert_eq!(sink.of(ProbeSubject::Model(fast.id)).status, TestStatus::Ok);
    assert_eq!(
        sink.of(ProbeSubject::Model(slow.id)).status,
        TestStatus::Timeout
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn models_are_probed_four_at_a_time_and_all_of_them_finish() {
    let server = Server::new(
        listing_of(r#"{"id":"m"}"#),
        Arc::new(|_| {
            std::thread::sleep(Duration::from_millis(50));
            ok(PLAIN_ANSWER)
        }),
    );
    let address = server.start().await;
    let sink = Collected::default();
    let models: Vec<_> = (1..=9).map(|number| model(number, "m")).collect();
    probe()
        .probe(&connection(address, None), &models, &sink)
        .await;
    for target in &models {
        assert_eq!(
            sink.of(ProbeSubject::Model(target.id)).status,
            TestStatus::Ok
        );
    }
    assert!(server.most_running.load(Ordering::SeqCst) <= 4);
}

#[tokio::test]
async fn results_arrive_one_at_a_time_so_progress_is_visible() {
    let server = Server::new(listing_of(r#"{"id":"m"}"#), chat_with_tools());
    let address = server.start().await;
    let sink = Collected::default();
    probe()
        .probe(&connection(address, None), &[model(1, "m")], &sink)
        .await;
    let order: Vec<_> = sink
        .all()
        .into_iter()
        .map(|result| result.subject)
        .collect();
    assert_eq!(
        order[0],
        ProbeSubject::Provider,
        "the provider result lands first"
    );
    assert_eq!(order.len(), 2);
}
