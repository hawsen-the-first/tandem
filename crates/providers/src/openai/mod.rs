//! The OpenAI-compatible provider: probe operations for the Test button (LC-26).
//!
//! One test lists the provider's models (which also shows whether the key works and, for some
//! providers, each model's context length), then for every configured model sends a small chat
//! request and a tool-call request that forces a trivial `echo` function (A5, A6). Outcomes map to
//! fixed statuses; nothing the remote server wrote is kept or shown (SE-22).

use async_trait::async_trait;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tandem_core::config::{ProbeDetails, ProbeResult, ProbeSubject, TestStatus, ToolCalling};
use tandem_outbound::{OutboundClient, OutboundError, Request, Response, Target};
use tandem_ports::{ModelToProbe, ProviderConnection, ProviderProbe, ResultSink};
use tokio::time::Instant;

/// Time for a whole test (BR-14.7).
const DEFAULT_BUDGET: Duration = Duration::from_secs(60);
/// Models probed at once.
const DEFAULT_CONCURRENCY: usize = 4;
/// Fields a provider's model listing may use for the context length (A6).
const CONTEXT_FIELDS: [&str; 3] = ["context_length", "max_model_len", "context_window"];

/// The probe for `kind = openai_compatible`.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProbe {
    client: OutboundClient,
    budget: Duration,
    concurrency: usize,
}

enum Listing {
    /// The listing worked; context lengths by model name.
    Listed(HashMap<String, u32>),
    /// The provider offers no usable listing; probing the models still tells us what we need.
    Unavailable,
    /// The provider cannot be used at all, so no model is probed.
    Failed(TestStatus),
}

impl OpenAiCompatibleProbe {
    /// A probe with the 60 second budget and four models at a time.
    pub fn new(client: OutboundClient) -> Self {
        Self {
            client,
            budget: DEFAULT_BUDGET,
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    /// Change the total time allowed for one test.
    #[must_use]
    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    async fn call(
        &self,
        connection: &ProviderConnection,
        path: &str,
        request: Request,
    ) -> Result<Response, OutboundError> {
        let request = match &connection.api_key {
            Some(key) => request.bearer(key.expose())?,
            None => request,
        };
        self.client
            .send(Target::Provider, &connection.base_url, path, request)
            .await
    }

    async fn list(&self, connection: &ProviderConnection) -> Listing {
        let response = match self.call(connection, "/models", Request::get()).await {
            Ok(response) => response,
            Err(error) => return Listing::Failed(error.test_status()),
        };
        if matches!(response.status, 401 | 403) {
            return Listing::Failed(TestStatus::Unauthorised);
        }
        if !response.is_success() {
            return Listing::Unavailable;
        }
        let Ok(body) = serde_json::from_slice::<Value>(&response.body) else {
            return Listing::Unavailable;
        };
        let Some(entries) = body.get("data").and_then(Value::as_array) else {
            return Listing::Unavailable;
        };
        let contexts = entries
            .iter()
            .filter_map(|entry| {
                let name = entry.get("id")?.as_str()?.to_owned();
                let tokens = CONTEXT_FIELDS
                    .iter()
                    .find_map(|field| entry.get(*field)?.as_u64())
                    .and_then(|tokens| u32::try_from(tokens).ok())
                    .filter(|tokens| *tokens > 0)?;
                Some((name, tokens))
            })
            .collect();
        Listing::Listed(contexts)
    }

    async fn probe_known(
        &self,
        connection: &ProviderConnection,
        model: &ModelToProbe,
        known: Option<&HashMap<String, u32>>,
    ) -> ProbeResult {
        let tokens = known.and_then(|contexts| contexts.get(model.name.as_str()).copied());
        self.probe_model(connection, model, tokens).await
    }

    async fn probe_model(
        &self,
        connection: &ProviderConnection,
        model: &ModelToProbe,
        context_tokens: Option<u32>,
    ) -> ProbeResult {
        let subject = ProbeSubject::Model(model.id);
        let chat = json!({
            "model": model.name.as_str(),
            "messages": [{"role": "user", "content": "Reply with the single word: ok"}],
            "max_tokens": 16,
            "stream": false,
        });
        let status = match self.post_chat(connection, &chat).await {
            Ok(response) if response.is_success() => {
                if has_choices(&response) {
                    TestStatus::Ok
                } else {
                    TestStatus::BadResponse
                }
            }
            Ok(response) => failure_status(&response),
            Err(error) => error.test_status(),
        };
        let mut result = ProbeResult::new(subject, status);
        if status == TestStatus::Ok {
            result.details = ProbeDetails {
                tool_calling: Some(self.probe_tools(connection, model).await),
                context_tokens,
                ..ProbeDetails::default()
            };
        }
        result
    }

    async fn probe_tools(
        &self,
        connection: &ProviderConnection,
        model: &ModelToProbe,
    ) -> ToolCalling {
        let body = json!({
            "model": model.name.as_str(),
            "messages": [{"role": "user", "content": "Call the echo function with the text ok."}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "echo",
                    "description": "Return the given text.",
                    "parameters": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"],
                    },
                },
            }],
            "tool_choice": {"type": "function", "function": {"name": "echo"}},
            "max_tokens": 64,
            "stream": false,
        });
        match self.post_chat(connection, &body).await {
            Ok(response) if response.is_success() => tool_calling_from(&response),
            Ok(response) if matches!(response.status, 400 | 422 | 501) => ToolCalling::Unsupported,
            Ok(_) | Err(_) => ToolCalling::Unknown,
        }
    }

    async fn post_chat(
        &self,
        connection: &ProviderConnection,
        body: &Value,
    ) -> Result<Response, OutboundError> {
        let bytes = serde_json::to_vec(body).map_err(|_| OutboundError::InvalidRequest)?;
        self.call(connection, "/chat/completions", Request::post_json(bytes))
            .await
    }
}

fn has_choices(response: &Response) -> bool {
    serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|body| body.get("choices")?.as_array().map(|list| !list.is_empty()))
        .unwrap_or(false)
}

fn tool_calling_from(response: &Response) -> ToolCalling {
    let Ok(body) = serde_json::from_slice::<Value>(&response.body) else {
        return ToolCalling::Unknown;
    };
    let Some(message) = body.pointer("/choices/0/message") else {
        return ToolCalling::Unknown;
    };
    let called_echo = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|calls| {
            calls
                .iter()
                .any(|call| call.pointer("/function/name").and_then(Value::as_str) == Some("echo"))
        });
    if called_echo {
        ToolCalling::Supported
    } else {
        ToolCalling::Unsupported
    }
}

/// The status for an error answer to a chat request.
fn failure_status(response: &Response) -> TestStatus {
    match response.status {
        401 | 403 => TestStatus::Unauthorised,
        404 => TestStatus::UnknownModel,
        400 | 422 => {
            let named_unknown = serde_json::from_slice::<Value>(&response.body)
                .ok()
                .and_then(|body| {
                    body.pointer("/error/code")
                        .and_then(Value::as_str)
                        .map(|code| code == "model_not_found")
                })
                .unwrap_or(false);
            if named_unknown {
                TestStatus::UnknownModel
            } else {
                TestStatus::BadResponse
            }
        }
        _ => TestStatus::BadResponse,
    }
}

#[async_trait]
impl ProviderProbe for OpenAiCompatibleProbe {
    async fn probe(
        &self,
        connection: &ProviderConnection,
        models: &[ModelToProbe],
        sink: &dyn ResultSink,
    ) {
        let deadline = Instant::now() + self.budget;
        let listing = tokio::time::timeout_at(deadline, self.list(connection))
            .await
            .unwrap_or(Listing::Failed(TestStatus::Timeout));
        let contexts = match listing {
            Listing::Failed(status) => {
                sink.record(ProbeResult::new(ProbeSubject::Provider, status))
                    .await;
                for model in models {
                    sink.record(ProbeResult::new(ProbeSubject::Model(model.id), status))
                        .await;
                }
                return;
            }
            Listing::Listed(contexts) => {
                sink.record(ProbeResult::new(ProbeSubject::Provider, TestStatus::Ok))
                    .await;
                Some(contexts)
            }
            Listing::Unavailable => None,
        };

        let known = contexts.as_ref();
        let mut waiting = models.iter();
        let mut running = FuturesUnordered::new();

        let mut finished = Vec::with_capacity(models.len());
        let mut provider_answered = false;
        let mut first_failure = None;
        loop {
            while running.len() < self.concurrency {
                let Some(model) = waiting.next() else { break };
                running.push(self.probe_known(connection, model, known));
            }
            if running.is_empty() {
                break;
            }
            match tokio::time::timeout_at(deadline, running.next()).await {
                Ok(Some(result)) => {
                    match result.status {
                        TestStatus::Ok | TestStatus::UnknownModel => provider_answered = true,
                        status => {
                            first_failure.get_or_insert(status);
                        }
                    }
                    finished.push(result.subject);
                    sink.record(result).await;
                }
                Ok(None) => break,
                Err(_) => {
                    for model in models
                        .iter()
                        .filter(|model| !finished.contains(&ProbeSubject::Model(model.id)))
                    {
                        sink.record(ProbeResult::new(
                            ProbeSubject::Model(model.id),
                            TestStatus::Timeout,
                        ))
                        .await;
                    }
                    first_failure.get_or_insert(TestStatus::Timeout);
                    break;
                }
            }
        }

        if contexts.is_none() {
            let status = if provider_answered {
                TestStatus::Ok
            } else {
                first_failure.unwrap_or(TestStatus::BadResponse)
            };
            sink.record(ProbeResult::new(ProbeSubject::Provider, status))
                .await;
        }
    }
}

#[cfg(test)]
mod tests;
