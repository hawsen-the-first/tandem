use super::*;
use axum::Router;
use axum::extract::Request as AxumRequest;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tandem_core::config::TestStatus;
use tandem_core::outbound_url::OutboundUrl;
use tokio::net::TcpListener;

async fn serve(router: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    address
}

fn base(address: SocketAddr) -> OutboundUrl {
    OutboundUrl::parse(&format!("http://{address}")).unwrap()
}

fn client() -> OutboundClient {
    OutboundClient::new().unwrap()
}

fn fast_client(total: Duration, max_bytes: usize) -> OutboundClient {
    OutboundClient::with(
        Limits {
            connect_timeout: Duration::from_millis(500),
            total_timeout: total,
            max_response_bytes: max_bytes,
            max_redirects: 3,
        },
        Arc::new(SystemLookup),
    )
    .unwrap()
}

/// A resolver whose answer the test can change, to stand in for DNS rebinding.
#[derive(Default)]
struct ScriptedLookup {
    answer: Mutex<Vec<IpAddr>>,
}

impl ScriptedLookup {
    fn set(&self, addresses: &[&str]) {
        *self.answer.lock().unwrap() = addresses.iter().map(|a| a.parse().unwrap()).collect();
    }
}

impl NameLookup for ScriptedLookup {
    fn lookup<'a>(
        &'a self,
        _host: &'a str,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<IpAddr>>> + Send + 'a>> {
        let answer = self.answer.lock().unwrap().clone();
        Box::pin(async move { Ok(answer) })
    }
}

// ---- the basics --------------------------------------------------------------------------------

#[tokio::test]
async fn a_get_returns_status_headers_and_body() {
    let address = serve(Router::new().route(
        "/models",
        get(|| async { ([("x-note", "hello")], "{\"data\":[]}") }),
    ))
    .await;
    let response = client()
        .send(Target::Provider, &base(address), "/models", Request::get())
        .await
        .unwrap();
    assert!(response.is_success());
    assert_eq!(response.status, 200);
    assert_eq!(response.header("x-note"), Some("hello"));
    assert_eq!(response.body, b"{\"data\":[]}");
}

#[tokio::test]
async fn an_http_error_status_is_a_response_not_an_error() {
    let address =
        serve(Router::new().route("/x", get(|| async { StatusCode::UNAUTHORIZED }))).await;
    let response = client()
        .send(Target::Provider, &base(address), "/x", Request::get())
        .await
        .unwrap();
    assert_eq!(response.status, 401);
    assert!(!response.is_success());
}

#[tokio::test]
async fn a_post_sends_its_json_body_and_a_sensitive_bearer_header() {
    let address = serve(Router::new().route(
        "/chat",
        post(|headers: HeaderMap, body: String| async move {
            let auth = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let kind = headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            format!("{auth}|{kind}|{body}")
        }),
    ))
    .await;
    let request = Request::post_json(b"{\"a\":1}".to_vec())
        .bearer("sk-secret")
        .unwrap();
    assert!(
        !format!("{request:?}").contains("sk-secret"),
        "the token never prints"
    );
    let response = client()
        .send(Target::Provider, &base(address), "/chat", request)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(response.body).unwrap(),
        "Bearer sk-secret|application/json|{\"a\":1}"
    );
}

#[tokio::test]
async fn the_user_agent_names_tandem_and_no_cookies_are_kept() {
    let address = serve(
        Router::new()
            .route(
                "/set",
                get(|| async { ([(header::SET_COOKIE, "session=abc")], "ok") }),
            )
            .route(
                "/echo",
                get(|headers: HeaderMap| async move {
                    let cookie = headers.contains_key(header::COOKIE);
                    let agent = headers
                        .get(header::USER_AGENT)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_owned();
                    format!("{cookie}|{agent}")
                }),
            ),
    )
    .await;
    let client = client();
    client
        .send(Target::Search, &base(address), "/set", Request::get())
        .await
        .unwrap();
    let echoed = client
        .send(Target::Search, &base(address), "/echo", Request::get())
        .await
        .unwrap();
    let text = String::from_utf8(echoed.body).unwrap();
    assert!(text.starts_with("false|Tandem/"), "{text}");
}

// ---- request URLs ------------------------------------------------------------------------------

#[test]
fn a_request_path_cannot_change_the_host_the_url_was_validated_for() {
    let base = OutboundUrl::parse("https://api.example.test/v1").unwrap();
    assert_eq!(
        build_request_url(&base, "/models?x=1").unwrap().as_str(),
        "https://api.example.test/v1/models?x=1"
    );
    for bad in ["models", "@evil.test/x", "/x#frag", "\\\\evil.test", ""] {
        assert!(build_request_url(&base, bad).is_err(), "{bad:?}");
    }
    // A backslash or `@` after a slash stays inside the path; the host is unchanged.
    let url = build_request_url(&base, "/\\@evil.test/x").unwrap();
    assert_eq!(url.host_str(), Some("api.example.test"));
}

// ---- limits ------------------------------------------------------------------------------------

#[tokio::test]
async fn a_response_over_the_cap_is_too_large_whether_declared_or_streamed() {
    let declared = serve(Router::new().route("/big", get(|| async { vec![b'x'; 2000] }))).await;
    let error = fast_client(Duration::from_secs(5), 1000)
        .send(Target::Provider, &base(declared), "/big", Request::get())
        .await
        .unwrap_err();
    assert_eq!(error, OutboundError::TooLarge);
    assert_eq!(error.test_status(), TestStatus::BadResponse);

    let streamed = serve(Router::new().route(
        "/stream",
        get(|| async {
            let parts = vec![vec![b'x'; 600]; 3]
                .into_iter()
                .map(|part| Ok::<_, io::Error>(axum::body::Bytes::from(part)));
            axum::body::Body::from_stream(futures_util::stream::iter(parts))
        }),
    ))
    .await;
    let error = fast_client(Duration::from_secs(5), 1000)
        .send(Target::Provider, &base(streamed), "/stream", Request::get())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        OutboundError::TooLarge,
        "no Content-Length, still capped"
    );

    let fits = fast_client(Duration::from_secs(5), 3000)
        .send(Target::Provider, &base(declared), "/big", Request::get())
        .await
        .unwrap();
    assert_eq!(fits.body.len(), 2000);
}

#[tokio::test]
async fn a_slow_answer_times_out() {
    let address = serve(Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            "late"
        }),
    ))
    .await;
    let error = fast_client(Duration::from_millis(300), 1000)
        .send(Target::Provider, &base(address), "/slow", Request::get())
        .await
        .unwrap_err();
    assert_eq!(error, OutboundError::Timeout);
    assert_eq!(error.test_status(), TestStatus::Timeout);
}

#[tokio::test]
async fn a_closed_port_is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let error = client()
        .send(Target::Provider, &base(address), "/", Request::get())
        .await
        .unwrap_err();
    assert_eq!(error, OutboundError::Unreachable);
    assert_eq!(error.test_status(), TestStatus::Unreachable);
}

#[tokio::test]
async fn https_to_a_server_that_does_not_speak_tls_is_unreachable_never_downgraded() {
    let address = serve(Router::new().route("/", get(|| async { "plain" }))).await;
    let url = OutboundUrl::parse(&format!("https://{address}")).unwrap();
    let error = client()
        .send(Target::Provider, &url, "/", Request::get())
        .await
        .unwrap_err();
    assert_eq!(error, OutboundError::Unreachable);
}

// ---- redirects (BR-19.5) -----------------------------------------------------------------------

#[tokio::test]
async fn a_redirect_on_the_same_scheme_host_and_port_is_followed() {
    let address = serve(
        Router::new()
            .route("/a", get(|| async { Redirect::temporary("/b?x=1") }))
            .route(
                "/b",
                get(|request: AxumRequest| async move {
                    request.uri().query().unwrap_or("").to_owned()
                }),
            ),
    )
    .await;
    let response = client()
        .send(Target::Search, &base(address), "/a", Request::get())
        .await
        .unwrap();
    assert_eq!(String::from_utf8(response.body).unwrap(), "x=1");
}

#[tokio::test]
async fn a_redirect_to_another_host_or_port_is_refused() {
    let other = serve(Router::new().route("/", get(|| async { "other" }))).await;
    let to_other_port = format!("http://127.0.0.1:{}/", other.port());
    let to_other_host = format!("http://localhost:{}/", other.port());
    let to_metadata = "http://169.254.169.254/latest/meta-data".to_owned();
    for location in [to_other_port, to_other_host, to_metadata] {
        let target = location.clone();
        let address = serve(Router::new().route(
            "/go",
            get(move || {
                let target = target.clone();
                async move { Redirect::temporary(&target) }
            }),
        ))
        .await;
        let error = client()
            .send(Target::Provider, &base(address), "/go", Request::get())
            .await
            .unwrap_err();
        assert_eq!(error, OutboundError::BadRedirect, "{location}");
        assert_eq!(error.test_status(), TestStatus::BadResponse);
    }
}

#[tokio::test]
async fn a_scheme_change_or_a_loop_or_a_redirect_without_a_location_is_refused() {
    let address = serve(
        Router::new()
            .route(
                "/to-https",
                get(|request: AxumRequest| async move {
                    let host = request
                        .headers()
                        .get(header::HOST)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_owned();
                    Redirect::temporary(&format!("https://{host}/x"))
                }),
            )
            .route("/loop", get(|| async { Redirect::temporary("/loop") }))
            .route("/bare", get(|| async { StatusCode::FOUND.into_response() })),
    )
    .await;
    for path in ["/to-https", "/loop", "/bare"] {
        let error = client()
            .send(Target::Provider, &base(address), path, Request::get())
            .await
            .unwrap_err();
        assert_eq!(error, OutboundError::BadRedirect, "{path}");
    }
}

#[tokio::test]
async fn at_most_three_redirects_are_followed() {
    let address = serve(
        Router::new()
            .route("/r0", get(|| async { Redirect::temporary("/r1") }))
            .route("/r1", get(|| async { Redirect::temporary("/r2") }))
            .route("/r2", get(|| async { Redirect::temporary("/r3") }))
            .route("/r3", get(|| async { "arrived" }))
            .route("/s0", get(|| async { Redirect::temporary("/s1") }))
            .route("/s1", get(|| async { Redirect::temporary("/s2") }))
            .route("/s2", get(|| async { Redirect::temporary("/s3") }))
            .route("/s3", get(|| async { Redirect::temporary("/s4") }))
            .route("/s4", get(|| async { "too far" })),
    )
    .await;
    let three = client()
        .send(Target::Provider, &base(address), "/r0", Request::get())
        .await
        .unwrap();
    assert_eq!(three.body, b"arrived");
    let four = client()
        .send(Target::Provider, &base(address), "/s0", Request::get())
        .await
        .unwrap_err();
    assert_eq!(four, OutboundError::BadRedirect);
}

#[tokio::test]
async fn a_307_keeps_the_method_and_body_and_a_302_turns_a_post_into_a_get() {
    let address = serve(
        Router::new()
            .route("/keep", post(|| async { Redirect::temporary("/echo") }))
            .route(
                "/change",
                post(|| async { Redirect::to("/echo").into_response() }),
            )
            .route(
                "/echo",
                axum::routing::any(|request: AxumRequest| async move {
                    let method = request.method().to_string();
                    let body = axum::body::to_bytes(request.into_body(), 1000)
                        .await
                        .unwrap();
                    format!("{method}:{}", String::from_utf8_lossy(&body))
                }),
            ),
    )
    .await;
    let kept = client()
        .send(
            Target::Provider,
            &base(address),
            "/keep",
            Request::post_json(b"{\"k\":1}".to_vec()),
        )
        .await
        .unwrap();
    assert_eq!(String::from_utf8(kept.body).unwrap(), "POST:{\"k\":1}");
    let changed = client()
        .send(
            Target::Provider,
            &base(address),
            "/change",
            Request::post_json(b"{\"k\":1}".to_vec()),
        )
        .await
        .unwrap();
    assert_eq!(String::from_utf8(changed.body).unwrap(), "GET:");
}

// ---- name resolution (BR-19.4, SE-19) ----------------------------------------------------------

#[tokio::test]
async fn a_name_is_checked_each_time_so_rebinding_to_a_blocked_address_is_refused() {
    let address = serve(Router::new().route("/", get(|| async { "inside" }))).await;
    let lookup = Arc::new(ScriptedLookup::default());
    let client = OutboundClient::with(Limits::default(), lookup.clone()).unwrap();
    let url = OutboundUrl::parse(&format!("http://models.internal:{}", address.port())).unwrap();

    lookup.set(&["127.0.0.1"]);
    let first = client
        .send(Target::Provider, &url, "/", Request::get())
        .await
        .unwrap();
    assert_eq!(first.body, b"inside");

    for blocked in [
        &["169.254.169.254"][..],
        &["fe80::1"],
        &["fd00:ec2::254"],
        &["::ffff:169.254.169.254"],
        &["100.100.100.200"],
        &["127.0.0.1", "169.254.169.254"],
        &["169.254.169.254", "127.0.0.1"],
    ] {
        lookup.set(blocked);
        let error = client
            .send(Target::Provider, &url, "/", Request::get())
            .await
            .unwrap_err();
        assert_eq!(error, OutboundError::RefusedAddress, "{blocked:?}");
        assert_eq!(error.test_status(), TestStatus::RefusedAddress);
    }

    lookup.set(&["127.0.0.1"]);
    assert!(
        client
            .send(Target::Provider, &url, "/", Request::get())
            .await
            .is_ok(),
        "allowed again"
    );
}

#[tokio::test]
async fn a_name_that_does_not_resolve_is_unreachable() {
    let lookup = Arc::new(ScriptedLookup::default());
    lookup.set(&[]);
    let client = OutboundClient::with(Limits::default(), lookup).unwrap();
    let url = OutboundUrl::parse("http://nowhere.internal:9").unwrap();
    let error = client
        .send(Target::Provider, &url, "/", Request::get())
        .await
        .unwrap_err();
    assert_eq!(error, OutboundError::Unreachable);
}

#[tokio::test]
async fn private_and_loopback_names_are_allowed() {
    let address = serve(Router::new().route("/", get(|| async { "ok" }))).await;
    let lookup = Arc::new(ScriptedLookup::default());
    let client = OutboundClient::with(Limits::default(), lookup.clone()).unwrap();
    let url = OutboundUrl::parse(&format!("http://ollama:{}", address.port())).unwrap();
    for allowed in ["127.0.0.1", "::1"] {
        lookup.set(&[allowed]);
        let result = client
            .send(Target::Provider, &url, "/", Request::get())
            .await;
        if allowed == "127.0.0.1" {
            assert!(result.is_ok());
        } else {
            // The mock server listens on IPv4 only, so ::1 is refused by the network, not the rules.
            assert_ne!(result.unwrap_err(), OutboundError::RefusedAddress);
        }
    }
}

// ---- metrics and errors ------------------------------------------------------------------------

#[tokio::test]
async fn calls_are_counted_by_target_kind_and_outcome_never_by_host() {
    // A process-wide recorder; only this test uses the `runtime` and `github` labels, so counts are exact.
    let handle = tandem_telemetry::MetricsRegistry::new("test")
        .unwrap()
        .install()
        .unwrap();
    let address = serve(Router::new().route("/", get(|| async { "ok" }))).await;
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        address
    };
    let client = client();
    client
        .send(Target::Runtime, &base(address), "/", Request::get())
        .await
        .unwrap();
    client
        .send(Target::Github, &base(closed), "/", Request::get())
        .await
        .unwrap_err();
    let text = handle.render();
    assert!(
        text.contains("outbound_requests_total{target=\"runtime\",outcome=\"ok\"} 1"),
        "{text}"
    );
    assert!(
        text.contains("outbound_requests_total{target=\"github\",outcome=\"unreachable\"} 1"),
        "{text}"
    );
    assert!(
        text.contains("outbound_request_duration_seconds_count{target=\"runtime\"} 1"),
        "{text}"
    );
    assert!(!text.contains("127.0.0.1"), "no host names in metrics");
}

#[test]
fn every_error_maps_to_a_fixed_test_status() {
    for (error, status) in [
        (OutboundError::RefusedAddress, TestStatus::RefusedAddress),
        (OutboundError::Unreachable, TestStatus::Unreachable),
        (OutboundError::InvalidRequest, TestStatus::Unreachable),
        (OutboundError::Timeout, TestStatus::Timeout),
        (OutboundError::TooLarge, TestStatus::BadResponse),
        (OutboundError::BadRedirect, TestStatus::BadResponse),
    ] {
        assert_eq!(error.test_status(), status);
        assert!(!error.to_string().is_empty());
    }
    assert_eq!(Target::Provider.as_str(), "provider");
    assert_eq!(Target::Github.as_str(), "github");
}
