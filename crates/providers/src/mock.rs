//! A deterministic, scripted model provider for the walking skeleton and tests (BR-13.5, NFR-7.3).

use async_trait::async_trait;
use tandem_ports::{ModelProvider, ModelRequest, ModelResponse, ProviderError};

/// How many characters of the prompt the default reply echoes.
const ECHO_CHARS: usize = 200;

/// A provider that answers from a script. The same input always gives the same reply.
#[derive(Debug, Clone, Default)]
pub struct MockProvider {
    scripts: Vec<(String, String)>,
    failing: bool,
}

impl MockProvider {
    /// A provider whose default reply is `Received: <first 200 characters of the prompt>`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reply with `reply` to any prompt containing `needle` (the first matching script wins).
    #[must_use]
    pub fn with_script(mut self, needle: &str, reply: &str) -> Self {
        self.scripts.push((needle.to_owned(), reply.to_owned()));
        self
    }

    /// A provider that always fails, for failure-path tests.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            scripts: Vec::new(),
            failing: true,
        }
    }
}

#[async_trait]
impl ModelProvider for MockProvider {
    fn name(&self) -> &str {
        "mock-agent"
    }

    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError> {
        if self.failing {
            return Err(ProviderError::Unavailable);
        }
        if let Some((_, reply)) = self
            .scripts
            .iter()
            .find(|(needle, _)| request.prompt.contains(needle.as_str()))
        {
            return Ok(ModelResponse {
                text: reply.clone(),
            });
        }
        let echoed: String = request.prompt.chars().take(ECHO_CHARS).collect();
        Ok(ModelResponse {
            text: format!("Received: {echoed}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(text: &str) -> ModelRequest {
        ModelRequest {
            prompt: text.to_owned(),
        }
    }

    #[tokio::test]
    async fn br_13_5_the_default_reply_echoes_the_first_200_characters() {
        let provider = MockProvider::new();
        let reply = provider.complete(&request("hello")).await.unwrap();
        assert_eq!(reply.text, "Received: hello");
        let long = "x".repeat(500);
        let reply = provider.complete(&request(&long)).await.unwrap();
        assert_eq!(reply.text, format!("Received: {}", "x".repeat(200)));
    }

    #[tokio::test]
    async fn br_13_5_the_same_input_always_gives_the_same_reply() {
        let provider = MockProvider::new();
        let first = provider.complete(&request("same")).await.unwrap();
        let second = provider.complete(&request("same")).await.unwrap();
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn scripts_override_the_default_and_the_first_match_wins() {
        let provider = MockProvider::new()
            .with_script("price", "It costs nothing.")
            .with_script("cost", "Never reached for this prompt.");
        let reply = provider
            .complete(&request("what is the price and cost?"))
            .await
            .unwrap();
        assert_eq!(reply.text, "It costs nothing.");
    }

    #[tokio::test]
    async fn a_failing_provider_reports_unavailable() {
        let result = MockProvider::failing().complete(&request("x")).await;
        assert_eq!(result, Err(ProviderError::Unavailable));
    }

    #[test]
    fn the_author_name_is_mock_agent() {
        assert_eq!(MockProvider::new().name(), "mock-agent");
    }
}
