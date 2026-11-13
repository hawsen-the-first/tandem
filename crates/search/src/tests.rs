use super::*;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tandem_support::Secret;
use tokio::net::TcpListener;

type Seen = Arc<Mutex<Vec<String>>>;

#[derive(Clone)]
struct Canned {
    status: StatusCode,
    body: String,
    seen: Seen,
}

async fn start(status: u16, body: &str) -> (SocketAddr, Seen) {
    let seen = Seen::default();
    let canned = Canned {
        status: StatusCode::from_u16(status).unwrap(),
        body: body.to_owned(),
        seen: seen.clone(),
    };
    async fn searx(
        State(canned): State<Canned>,
        Query(query): Query<HashMap<String, String>>,
    ) -> (StatusCode, String) {
        canned
            .seen
            .lock()
            .unwrap()
            .push(format!("q={} format={}", query["q"], query["format"]));
        (canned.status, canned.body)
    }
    async fn brave(
        State(canned): State<Canned>,
        headers: HeaderMap,
        Query(query): Query<HashMap<String, String>>,
    ) -> (StatusCode, String) {
        let token = headers
            .get("x-subscription-token")
            .map(|value| value.to_str().unwrap().to_owned());
        canned
            .seen
            .lock()
            .unwrap()
            .push(format!("q={} token={token:?}", query["q"]));
        (canned.status, canned.body)
    }
    async fn tavily(
        State(canned): State<Canned>,
        headers: HeaderMap,
        body: String,
    ) -> (StatusCode, String) {
        let auth = headers
            .get("authorization")
            .map(|value| value.to_str().unwrap().to_owned());
        canned.seen.lock().unwrap().push(format!("{auth:?} {body}"));
        (canned.status, canned.body)
    }
    let router = Router::new()
        .route("/search", get(searx).post(tavily))
        .route("/res/v1/web/search", get(brave))
        .with_state(canned);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, seen)
}

fn url(address: SocketAddr) -> OutboundUrl {
    OutboundUrl::parse(&format!("http://{address}")).unwrap()
}

fn tester(address: SocketAddr) -> SearchTester {
    SearchTester::new(OutboundClient::new().unwrap()).with_endpoints(url(address), url(address))
}

fn connection(kind: SearchKind, address: SocketAddr, key: Option<&str>) -> SearchConnection {
    SearchConnection {
        kind,
        base_url: kind.needs_url().then(|| url(address)),
        api_key: key.map(Secret::new),
    }
}

const WITH_RESULTS: &str = r#"{"results":[{"title":"x","url":"https://example.com"}]}"#;
const BRAVE_RESULTS: &str = r#"{"web":{"results":[{"title":"x"}]}}"#;

#[tokio::test]
async fn searxng_asks_for_json_and_succeeds_on_results() {
    let (address, seen) = start(200, WITH_RESULTS).await;
    let result = tester(address)
        .test(&connection(SearchKind::Searxng, address, None))
        .await;
    assert_eq!(result.subject, ProbeSubject::Search);
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.note, None);
    assert_eq!(seen.lock().unwrap().as_slice(), ["q=test format=json"]);
}

#[tokio::test]
async fn no_results_is_a_success_with_a_note() {
    for (kind, body) in [
        (SearchKind::Searxng, r#"{"results":[]}"#),
        (SearchKind::Brave, r#"{"web":{"results":[]}}"#),
        (SearchKind::Tavily, r#"{"results":[]}"#),
    ] {
        let (address, _) = start(200, body).await;
        let result = tester(address)
            .test(&connection(kind, address, Some("key")))
            .await;
        assert_eq!(result.status, TestStatus::Ok, "{kind:?}");
        assert_eq!(result.note, Some(ProbeNote::NoResults), "{kind:?}");
    }
}

#[tokio::test]
async fn searxng_with_json_switched_off_is_a_bad_response_not_a_key_problem() {
    let (address, _) = start(403, "Forbidden").await;
    let result = tester(address)
        .test(&connection(SearchKind::Searxng, address, None))
        .await;
    assert_eq!(result.status, TestStatus::BadResponse);
}

#[tokio::test]
async fn brave_sends_the_key_in_its_token_header() {
    let (address, seen) = start(200, BRAVE_RESULTS).await;
    let result = tester(address)
        .test(&connection(SearchKind::Brave, address, Some("brave-key")))
        .await;
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        [r#"q=test token=Some("brave-key")"#]
    );
}

#[tokio::test]
async fn tavily_sends_the_key_as_a_bearer_token_with_a_small_query() {
    let (address, seen) = start(200, WITH_RESULTS).await;
    let result = tester(address)
        .test(&connection(SearchKind::Tavily, address, Some("tavily-key")))
        .await;
    assert_eq!(result.status, TestStatus::Ok);
    let seen = seen.lock().unwrap();
    assert!(seen[0].contains(r#"Some("Bearer tavily-key")"#), "{seen:?}");
    assert!(seen[0].contains(r#""max_results":1"#), "{seen:?}");
}

#[tokio::test]
async fn a_refused_key_is_unauthorised_for_the_providers_that_use_keys() {
    for (kind, status) in [
        (SearchKind::Brave, 401),
        (SearchKind::Brave, 403),
        (SearchKind::Tavily, 401),
    ] {
        let (address, _) = start(status, r#"{"detail":"SECRET-REMOTE-TEXT"}"#).await;
        let result = tester(address)
            .test(&connection(kind, address, Some("bad")))
            .await;
        assert_eq!(result.status, TestStatus::Unauthorised, "{kind:?} {status}");
        assert!(!format!("{result:?}").contains("SECRET-REMOTE-TEXT"));
    }
}

#[tokio::test]
async fn other_failures_and_odd_answers_are_bad_responses() {
    for (status, body) in [
        (500, "boom"),
        (429, "slow down"),
        (200, "<html></html>"),
        (200, r#"{"nothing":true}"#),
    ] {
        let (address, _) = start(status, body).await;
        let result = tester(address)
            .test(&connection(SearchKind::Tavily, address, Some("key")))
            .await;
        assert_eq!(result.status, TestStatus::BadResponse, "{status} {body}");
    }
}

#[tokio::test]
async fn a_missing_url_or_key_is_not_configured_and_nothing_is_sent() {
    let (address, seen) = start(200, WITH_RESULTS).await;
    let no_url = SearchConnection {
        kind: SearchKind::Searxng,
        base_url: None,
        api_key: None,
    };
    assert_eq!(
        tester(address).test(&no_url).await.status,
        TestStatus::NotConfigured
    );
    for kind in [SearchKind::Brave, SearchKind::Tavily] {
        assert_eq!(
            tester(address)
                .test(&connection(kind, address, None))
                .await
                .status,
            TestStatus::NotConfigured
        );
    }
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_unreachable_server_is_reported_as_such() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let result = tester(closed)
        .test(&connection(SearchKind::Searxng, closed, None))
        .await;
    assert_eq!(result.status, TestStatus::Unreachable);
}

#[test]
fn the_real_endpoints_are_the_fixed_public_ones() {
    let tester = SearchTester::new(OutboundClient::new().unwrap());
    assert_eq!(tester.brave.as_str(), "https://api.search.brave.com");
    assert_eq!(tester.tavily.as_str(), "https://api.tavily.com");
}
