//! Probes: the operations behind the "Test" buttons (US-2.1, 2.3, 2.5, 2.6, 2.9).
//!
//! A probe never relays remote text: every outcome is a fixed [`TestStatus`] (SE-22). Connection
//! values hold their secrets in [`Secret`], which cannot print itself, and have no serialisation.

use async_trait::async_trait;
use std::fmt;
use tandem_core::config::{ModelName, ProbeResult, SearchKind};
use tandem_core::ids::ProviderModelId;
use tandem_core::outbound_url::OutboundUrl;
use tandem_support::Secret;

/// Where and how to reach one model provider.
#[derive(Clone)]
pub struct ProviderConnection {
    /// The base URL, including any `/v1`.
    pub base_url: OutboundUrl,
    /// The API key, when the provider needs one.
    pub api_key: Option<Secret>,
}

impl fmt::Debug for ProviderConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConnection")
            .field("base_url", &self.base_url.as_str())
            .field("api_key_set", &self.api_key.is_some())
            .finish()
    }
}

/// A model to probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelToProbe {
    /// Our identifier for the model.
    pub id: ProviderModelId,
    /// The name the provider's API expects.
    pub name: ModelName,
}

/// Where probe results go as they arrive, so a run shows progress and keeps partial results.
#[async_trait]
pub trait ResultSink: Send + Sync {
    /// Store one result.
    async fn record(&self, result: ProbeResult);
}

/// Test a model provider and its models.
#[async_trait]
pub trait ProviderProbe: Send + Sync {
    /// Probe the provider as a whole and then each model, sending every result to `sink`.
    async fn probe(
        &self,
        connection: &ProviderConnection,
        models: &[ModelToProbe],
        sink: &dyn ResultSink,
    );
}

/// How to reach the search provider that is being tested.
#[derive(Clone)]
pub struct SearchConnection {
    /// Which provider.
    pub kind: SearchKind,
    /// SearXNG's base URL; unused for the providers with fixed endpoints.
    pub base_url: Option<OutboundUrl>,
    /// The API key, for Brave and Tavily.
    pub api_key: Option<Secret>,
}

impl fmt::Debug for SearchConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchConnection")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url.as_ref().map(OutboundUrl::as_str))
            .field("api_key_set", &self.api_key.is_some())
            .finish()
    }
}

/// Test a search provider with one small query.
#[async_trait]
pub trait SearchProbe: Send + Sync {
    /// The result for the search subject.
    async fn test(&self, connection: &SearchConnection) -> ProbeResult;
}

/// The token for a connector.
#[derive(Clone)]
pub struct ConnectorConnection {
    /// The access token.
    pub token: Secret,
}

impl fmt::Debug for ConnectorConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorConnection")
            .field("token", &"redacted")
            .finish()
    }
}

/// Test a connector (GitHub).
#[async_trait]
pub trait ConnectorProbe: Send + Sync {
    /// The result for the connector subject.
    async fn test(&self, connection: &ConnectorConnection) -> ProbeResult;
}

/// Ask whether a container runtime is reachable (US-2.9).
#[async_trait]
pub trait RuntimeProbe: Send + Sync {
    /// The result for the runtime subject; `not_configured` when no socket is set.
    async fn probe(&self) -> ProbeResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_values_never_print_their_secrets() {
        let secret = "sk-test-do-not-print";
        let provider = ProviderConnection {
            base_url: OutboundUrl::parse("http://localhost:11434/v1").unwrap(),
            api_key: Some(Secret::new(secret)),
        };
        let search = SearchConnection {
            kind: SearchKind::Brave,
            base_url: None,
            api_key: Some(Secret::new(secret)),
        };
        let connector = ConnectorConnection {
            token: Secret::new(secret),
        };
        for printed in [
            format!("{provider:?}"),
            format!("{search:?}"),
            format!("{connector:?}"),
            format!("{provider:#?}"),
        ] {
            assert!(!printed.contains(secret), "{printed}");
        }
    }
}
