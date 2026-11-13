//! Web search adapters (SearXNG, Brave Search, Tavily). U2 uses only the test call: one small
//! query whose outcome becomes a fixed status (BR-16.4, SE-22). Searching for agents arrives in U4.

use async_trait::async_trait;
use serde_json::{Value, json};
use tandem_core::config::{ProbeNote, ProbeResult, ProbeSubject, SearchKind, TestStatus};
use tandem_core::outbound_url::OutboundUrl;
use tandem_outbound::{OutboundClient, OutboundError, Request, Response, Target};
use tandem_ports::{SearchConnection, SearchProbe};

/// The fixed Brave Search endpoint (BR-16.2).
const BRAVE_BASE: &str = "https://api.search.brave.com";
/// The fixed Tavily endpoint (BR-16.2).
const TAVILY_BASE: &str = "https://api.tavily.com";
/// The query sent by a test.
const TEST_QUERY: &str = "test";

/// Tests SearXNG (at the admin's URL), Brave Search and Tavily (at their fixed endpoints).
#[derive(Debug, Clone)]
pub struct SearchTester {
    client: OutboundClient,
    brave: OutboundUrl,
    tavily: OutboundUrl,
}

impl SearchTester {
    /// A tester that reaches the real Brave and Tavily endpoints.
    ///
    /// # Panics
    /// Never: the two endpoints are constants that parse.
    pub fn new(client: OutboundClient) -> Self {
        Self {
            client,
            brave: fixed(BRAVE_BASE),
            tavily: fixed(TAVILY_BASE),
        }
    }

    #[cfg(test)]
    fn with_endpoints(mut self, brave: OutboundUrl, tavily: OutboundUrl) -> Self {
        self.brave = brave;
        self.tavily = tavily;
        self
    }

    async fn call(
        &self,
        base: &OutboundUrl,
        path: &str,
        request: Request,
    ) -> Result<Response, OutboundError> {
        self.client.send(Target::Search, base, path, request).await
    }

    async fn run(&self, connection: &SearchConnection) -> Done {
        match connection.kind {
            SearchKind::Searxng => {
                let Some(base) = &connection.base_url else {
                    return Done::status(TestStatus::NotConfigured);
                };
                let outcome = self
                    .call(
                        base,
                        &format!("/search?q={TEST_QUERY}&format=json"),
                        Request::get(),
                    )
                    .await;
                judge(outcome, "/results", true)
            }
            SearchKind::Brave => {
                let Some(key) = &connection.api_key else {
                    return Done::status(TestStatus::NotConfigured);
                };
                let request = match Request::get()
                    .header("accept", "application/json", false)
                    .and_then(|request| request.header("x-subscription-token", key.expose(), true))
                {
                    Ok(request) => request,
                    Err(error) => return Done::status(error.test_status()),
                };
                let outcome = self
                    .call(
                        &self.brave,
                        &format!("/res/v1/web/search?q={TEST_QUERY}&count=1"),
                        request,
                    )
                    .await;
                judge(outcome, "/web/results", false)
            }
            SearchKind::Tavily => {
                let Some(key) = &connection.api_key else {
                    return Done::status(TestStatus::NotConfigured);
                };
                let body = json!({"query": TEST_QUERY, "max_results": 1});
                let Ok(bytes) = serde_json::to_vec(&body) else {
                    return Done::status(TestStatus::Unreachable);
                };
                let request = match Request::post_json(bytes).bearer(key.expose()) {
                    Ok(request) => request,
                    Err(error) => return Done::status(error.test_status()),
                };
                let outcome = self.call(&self.tavily, "/search", request).await;
                judge(outcome, "/results", false)
            }
        }
    }
}

/// A status and whether the search found nothing.
struct Done {
    status: TestStatus,
    empty: bool,
}

impl Done {
    fn status(status: TestStatus) -> Self {
        Self {
            status,
            empty: false,
        }
    }
}

fn fixed(text: &str) -> OutboundUrl {
    match OutboundUrl::parse(text) {
        Ok(url) => url,
        Err(_) => unreachable!("a constant search endpoint is a valid public https URL"),
    }
}

/// Turn the call's outcome into a status. `results_pointer` locates the results array in the JSON
/// answer. SearXNG answers 403 when its JSON format is switched off, which is not a key problem.
fn judge(
    outcome: Result<Response, OutboundError>,
    results_pointer: &str,
    forbidden_is_misconfiguration: bool,
) -> Done {
    let response = match outcome {
        Ok(response) => response,
        Err(error) => return Done::status(error.test_status()),
    };
    if !response.is_success() {
        let status = match response.status {
            401 | 403 if !(forbidden_is_misconfiguration && response.status == 403) => {
                TestStatus::Unauthorised
            }
            _ => TestStatus::BadResponse,
        };
        return Done::status(status);
    }
    let results = serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|body| {
            body.pointer(results_pointer)
                .and_then(Value::as_array)
                .map(Vec::len)
        });
    match results {
        Some(count) => Done {
            status: TestStatus::Ok,
            empty: count == 0,
        },
        None => Done::status(TestStatus::BadResponse),
    }
}

#[async_trait]
impl SearchProbe for SearchTester {
    async fn test(&self, connection: &SearchConnection) -> ProbeResult {
        let done = self.run(connection).await;
        let mut result = ProbeResult::new(ProbeSubject::Search, done.status);
        if done.status == TestStatus::Ok && done.empty {
            result.note = Some(ProbeNote::NoResults);
        }
        result
    }
}

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-search";

#[cfg(test)]
mod tests;
