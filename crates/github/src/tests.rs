use super::*;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response as AxumResponse};
use axum::routing::get;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tandem_support::Secret;
use tokio::net::TcpListener;

const TOKEN: &str = "github_pat_not_a_real_token";

#[derive(Clone)]
struct Fake {
    user: (u16, String, Option<&'static str>),
    pages: Arc<dyn Fn(u32) -> (u16, String, Option<String>) + Send + Sync>,
    log: Arc<Mutex<Vec<String>>>,
}

fn reply(status: u16, body: String, headers: &[(&'static str, String)]) -> AxumResponse {
    let mut response = (StatusCode::from_u16(status).unwrap(), body).into_response();
    for (name, value) in headers {
        response
            .headers_mut()
            .insert(*name, HeaderValue::from_str(value).unwrap());
    }
    response
}

async fn start(fake: Fake) -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
    async fn user(State(fake): State<Fake>, headers: HeaderMap) -> AxumResponse {
        fake.log.lock().unwrap().push(format!(
            "/user auth={} accept={} version={}",
            headers["authorization"].to_str().unwrap(),
            headers["accept"].to_str().unwrap(),
            headers["x-github-api-version"].to_str().unwrap()
        ));
        let (status, body, expiry) = fake.user.clone();
        let extra: Vec<_> = expiry
            .map(|value| ("github-authentication-token-expiration", value.to_owned()))
            .into_iter()
            .collect();
        reply(status, body, &extra)
    }
    async fn repos(
        State(fake): State<Fake>,
        Query(query): Query<HashMap<String, String>>,
    ) -> AxumResponse {
        let page: u32 = query["page"].parse().unwrap();
        fake.log.lock().unwrap().push(format!(
            "/user/repos per_page={} page={page}",
            query["per_page"]
        ));
        let (status, body, link) = (fake.pages)(page);
        let extra: Vec<_> = link.map(|value| ("link", value)).into_iter().collect();
        reply(status, body, &extra)
    }
    let log = fake.log.clone();
    let router = Router::new()
        .route("/user", get(user))
        .route("/user/repos", get(repos))
        .with_state(fake);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, log)
}

fn connector(address: SocketAddr) -> GithubConnector {
    GithubConnector::new(OutboundClient::new().unwrap())
        .with_api_base(OutboundUrl::parse(&format!("http://{address}")).unwrap())
}

fn connection() -> ConnectorConnection {
    ConnectorConnection {
        token: Secret::new(TOKEN),
    }
}

fn repos(from: u32, to: u32) -> String {
    let items: Vec<_> = (from..=to)
        .map(|number| format!(r#"{{"full_name":"acme/repo-{number}","private":true}}"#))
        .collect();
    format!("[{}]", items.join(","))
}

fn fake(
    user: (u16, &str, Option<&'static str>),
    pages: impl Fn(u32) -> (u16, String, Option<String>) + Send + Sync + 'static,
) -> Fake {
    Fake {
        user: (user.0, user.1.to_owned(), user.2),
        pages: Arc::new(pages),
        log: Arc::default(),
    }
}

const ME: &str = r#"{"login":"octocat","id":1}"#;

#[tokio::test]
async fn a_good_token_reports_the_account_the_expiry_and_the_repositories() {
    let (address, log) = start(fake((200, ME, Some("2026-12-01 10:30:00 UTC")), |_| {
        (200, repos(1, 3), None)
    }))
    .await;
    let result = connector(address).test(&connection()).await;
    assert_eq!(result.subject, ProbeSubject::Connector);
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.details.account_login.as_deref(), Some("octocat"));
    assert_eq!(
        result.details.token_expires_at,
        Some(time::macros::datetime!(2026-12-01 10:30:00 UTC))
    );
    assert_eq!(
        result.details.repositories,
        ["acme/repo-1", "acme/repo-2", "acme/repo-3"]
    );
    assert_eq!(result.details.repository_total, Some(3));
    let log = log.lock().unwrap();
    assert_eq!(
        log[0],
        format!("/user auth=Bearer {TOKEN} accept=application/vnd.github+json version=2022-11-28")
    );
    assert_eq!(log.len(), 2, "no extra calls for a single page");
}

#[tokio::test]
async fn a_token_that_never_expires_has_no_expiry() {
    let (address, _) = start(fake((200, ME, None), |_| (200, repos(1, 1), None))).await;
    let result = connector(address).test(&connection()).await;
    assert_eq!(result.details.token_expires_at, None);
}

#[tokio::test]
async fn many_repositories_are_counted_from_the_last_page_and_only_a_hundred_are_listed() {
    let (address, log) = start(fake((200, ME, None), |page| match page {
        1 => (
            200,
            repos(1, 100),
            Some(
                r#"<https://evil.example/user/repos?per_page=100&page=2>; rel="next", <https://evil.example/user/repos?per_page=100&page=3>; rel="last""#
                    .to_owned(),
            ),
        ),
        3 => (200, repos(201, 240), None),
        _ => (500, String::new(), None),
    }))
    .await;
    let result = connector(address).test(&connection()).await;
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.details.repositories.len(), 100);
    assert_eq!(result.details.repository_total, Some(240));
    let log = log.lock().unwrap();
    assert_eq!(
        log[1..],
        [
            "/user/repos per_page=100 page=1",
            "/user/repos per_page=100 page=3"
        ],
        "links in answers are read for a number, never followed"
    );
}

#[tokio::test]
async fn exactly_a_hundred_repositories_with_no_further_page_is_a_total_of_a_hundred() {
    let (address, _) = start(fake((200, ME, None), |_| (200, repos(1, 100), None))).await;
    let result = connector(address).test(&connection()).await;
    assert_eq!(result.details.repository_total, Some(100));
}

#[tokio::test]
async fn a_refused_token_is_unauthorised_and_nothing_else_is_asked() {
    for status in [401, 403] {
        let (address, log) = start(fake(
            (status, r#"{"message":"SECRET-REMOTE-TEXT"}"#, None),
            |_| (200, repos(1, 1), None),
        ))
        .await;
        let result = connector(address).test(&connection()).await;
        assert_eq!(result.status, TestStatus::Unauthorised, "{status}");
        assert_eq!(log.lock().unwrap().len(), 1);
        let printed = format!("{result:?}");
        assert!(!printed.contains("SECRET-REMOTE-TEXT") && !printed.contains(TOKEN));
    }
}

#[tokio::test]
async fn a_token_that_cannot_list_repositories_is_reported_as_refused() {
    let (address, _) = start(fake((200, ME, None), |_| (403, String::new(), None))).await;
    let result = connector(address).test(&connection()).await;
    assert_eq!(result.status, TestStatus::Unauthorised);
}

#[tokio::test]
async fn answers_that_are_not_what_github_sends_are_bad_responses() {
    for (user, pages_body) in [
        ((200, "<html></html>"), repos(1, 1)),
        ((200, r#"{"id":1}"#), repos(1, 1)),
        ((500, ""), repos(1, 1)),
        ((200, ME), r#"{"message":"not a list"}"#.to_owned()),
    ] {
        let body = pages_body.clone();
        let (address, _) = start(fake((user.0, user.1, None), move |_| {
            (200, body.clone(), None)
        }))
        .await;
        let result = connector(address).test(&connection()).await;
        assert_eq!(result.status, TestStatus::BadResponse, "{user:?}");
    }
}

#[tokio::test]
async fn an_unreachable_github_is_reported_as_such() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let result = connector(closed).test(&connection()).await;
    assert_eq!(result.status, TestStatus::Unreachable);
}

#[test]
fn the_real_api_base_is_fixed() {
    let connector = GithubConnector::new(OutboundClient::new().unwrap());
    assert_eq!(connector.api_base.as_str(), "https://api.github.com");
}

#[test]
fn the_expiry_header_and_the_link_header_are_parsed_strictly() {
    assert_eq!(
        parse_expiry("2026-01-02 03:04:05 UTC"),
        Some(time::macros::datetime!(2026-01-02 03:04:05 UTC))
    );
    for bad in [
        "",
        "tomorrow",
        "2026-01-02 03:04:05",
        "2026-13-02 03:04:05 UTC",
    ] {
        assert_eq!(parse_expiry(bad), None, "{bad}");
    }
    assert_eq!(
        last_page_number(r#"<https://api.github.com/user/repos?per_page=100&page=7>; rel="last""#),
        Some(7)
    );
    assert_eq!(
        last_page_number(r#"<https://api.github.com/user/repos?page=2>; rel="next""#),
        None
    );
    assert_eq!(last_page_number("garbage"), None);
}
