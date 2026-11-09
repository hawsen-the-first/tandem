//! Model provider port. U1 uses it only through the scripted mock; U4 widens it (tools,
//! streaming, capabilities).

use async_trait::async_trait;

/// A request to a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    /// The prompt text.
    pub prompt: String,
}

/// A model's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    /// The reply text.
    pub text: String,
}

/// Why a model call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider could not be reached or timed out.
    #[error("the model provider is unavailable")]
    Unavailable,
    /// Any other failure.
    #[error("the model provider failed: {0}")]
    Failed(String),
}

/// A source of model completions.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// The provider's name, shown as the author of agent messages.
    fn name(&self) -> &str;
    /// Complete a prompt.
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError>;
}
