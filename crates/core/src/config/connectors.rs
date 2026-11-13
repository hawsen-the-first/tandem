//! Search providers and the GitHub connector (BR-16, BR-17).

use crate::ids::UserId;
use crate::outbound_url::OutboundUrl;
use time::OffsetDateTime;

/// The address of the bundled optional SearXNG service in the reference Compose file (U8).
pub const DEFAULT_SEARXNG_URL: &str = "http://searxng:8080";

/// A search provider kind (BR-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    /// SearXNG, the default.
    Searxng,
    /// Brave Search.
    Brave,
    /// Tavily.
    Tavily,
}

impl SearchKind {
    /// Every kind, in the order shown.
    pub const ALL: [Self; 3] = [Self::Searxng, Self::Brave, Self::Tavily];

    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Searxng => "searxng",
            Self::Brave => "brave",
            Self::Tavily => "tavily",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }

    /// Whether the admin supplies the base URL (SearXNG only; the others have fixed endpoints).
    pub const fn needs_url(self) -> bool {
        matches!(self, Self::Searxng)
    }

    /// Whether an API key is needed (Brave and Tavily).
    pub const fn needs_key(self) -> bool {
        !matches!(self, Self::Searxng)
    }
}

/// The settings kept for one search kind; the key is sealed in the store (BR-16.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchConfig {
    /// Which provider.
    pub kind: SearchKind,
    /// SearXNG's base URL; none for the other kinds.
    pub base_url: Option<OutboundUrl>,
    /// When a key was last saved.
    pub key_saved_at: Option<OffsetDateTime>,
    /// Last changed.
    pub updated_at: OffsetDateTime,
    /// Last changed by.
    pub updated_by: UserId,
}

/// A connector kind (FR-12.1). GitHub is the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorKind {
    /// GitHub.
    Github,
}

impl ConnectorKind {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "github" => Some(Self::Github),
            _ => None,
        }
    }
}

/// How a connector authenticates. The GitHub App (`app`) arrives in U5 as US-2.13.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorAuth {
    /// A fine-grained personal access token.
    Token,
}

impl ConnectorAuth {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Token => "token",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "token" => Some(Self::Token),
            _ => None,
        }
    }
}

/// A configured connector; its secret is sealed in the store (BR-17.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// Which connector.
    pub kind: ConnectorKind,
    /// How it authenticates.
    pub auth: ConnectorAuth,
    /// When the secret was last saved.
    pub secret_saved_at: OffsetDateTime,
    /// The account the token belongs to, filled by a successful test.
    pub account_login: Option<String>,
    /// When the token expires, when GitHub reports it.
    pub token_expires_at: Option<OffsetDateTime>,
    /// Last changed.
    pub updated_at: OffsetDateTime,
    /// Last changed by.
    pub updated_by: UserId,
}
