//! Providers, models, capabilities, suitability and default models (BR-14, BR-15).

use super::{ConfigRuleError, clean_text};
use crate::ids::{ProviderId, ProviderModelId, UserId};
use crate::outbound_url::OutboundUrl;
use std::collections::HashSet;
use time::OffsetDateTime;

/// Longest provider name.
pub const MAX_PROVIDER_NAME_CHARS: usize = 60;
/// Longest model name.
pub const MAX_MODEL_NAME_CHARS: usize = 200;
/// Longest friendly model label.
pub const MAX_MODEL_LABEL_CHARS: usize = 80;
/// Design size: providers per instance (NFR SC-5).
pub const MAX_PROVIDERS: usize = 20;
/// Design size: models per provider.
pub const MAX_MODELS_PER_PROVIDER: usize = 100;
/// Largest context length an admin may declare.
pub const MAX_CONTEXT_TOKENS: u32 = 10_000_000;
/// A job needs at least this much context (A7).
pub const MIN_JOB_CONTEXT_TOKENS: u32 = 8_000;

/// What kind of server a provider is. Extensible: Anthropic (U8) and the Agent SDK (U9) follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// An OpenAI-compatible HTTP API (Ollama, LM Studio, vLLM, OpenRouter, OpenAI).
    OpenAiCompatible,
}

impl ProviderKind {
    /// Stable text form used in storage and the API.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "openai_compatible" => Some(Self::OpenAiCompatible),
            _ => None,
        }
    }
}

/// A provider's name: 1 to 60 characters, unique ignoring case (BR-14.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderName(String);

impl ProviderName {
    /// Validate and normalise (trim) a name.
    ///
    /// # Errors
    /// [`ConfigRuleError`] for an empty, long or control-character name.
    pub fn parse(text: &str) -> Result<Self, ConfigRuleError> {
        clean_text("The name", text, MAX_PROVIDER_NAME_CHARS).map(Self)
    }

    /// The name as entered (trimmed).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key two names are compared by: lower-cased.
    pub fn unique_key(&self) -> String {
        self.0.to_lowercase()
    }
}

/// The identifier a provider's API expects for a model: 1 to 200 characters (BR-14.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelName(String);

impl ModelName {
    /// Validate and normalise (trim) a model name.
    ///
    /// # Errors
    /// [`ConfigRuleError`] for an empty, long or control-character name.
    pub fn parse(text: &str) -> Result<Self, ConfigRuleError> {
        clean_text("The model name", text, MAX_MODEL_NAME_CHARS).map(Self)
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Check a provider's model list: at most the design size, no duplicates (BR-14.3).
///
/// # Errors
/// [`ConfigRuleError::TooMany`] or [`ConfigRuleError::DuplicateModel`].
pub fn validate_model_names(names: &[ModelName]) -> Result<(), ConfigRuleError> {
    if names.len() > MAX_MODELS_PER_PROVIDER {
        return Err(ConfigRuleError::TooMany {
            what: "Models per provider",
            max: MAX_MODELS_PER_PROVIDER,
        });
    }
    let mut seen = HashSet::new();
    if names.iter().all(|name| seen.insert(name.as_str())) {
        Ok(())
    } else {
        Err(ConfigRuleError::DuplicateModel)
    }
}

/// A provider. The API key is sealed in the store; `key_saved_at` records that one exists (BR-20.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// Identifier.
    pub id: ProviderId,
    /// Kind of server.
    pub kind: ProviderKind,
    /// Name shown to admins and owners.
    pub name: ProviderName,
    /// Base URL, including any `/v1` (BR-19).
    pub base_url: OutboundUrl,
    /// When a key was last saved; none when there is no key.
    pub key_saved_at: Option<OffsetDateTime>,
    /// Created.
    pub created_at: OffsetDateTime,
    /// Created by.
    pub created_by: UserId,
    /// Last changed.
    pub updated_at: OffsetDateTime,
    /// Last changed by.
    pub updated_by: UserId,
}

/// Whether a model can call tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCalling {
    /// It returned a tool call when asked to.
    Supported,
    /// The provider rejected tools or answered in plain text.
    Unsupported,
    /// Not tested, or the test was inconclusive.
    Unknown,
}

impl ToolCalling {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "supported" => Some(Self::Supported),
            "unsupported" => Some(Self::Unsupported),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// What a model can do. Detected and declared values are kept apart: a test replaces only the
/// detected ones and never touches what an admin declared (BR-14.5); the effective value is the
/// declared one when there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// Found by a test.
    pub detected_tool_calling: ToolCalling,
    /// Declared by an admin.
    pub declared_tool_calling: Option<ToolCalling>,
    /// Found by a test (from the provider's model listing).
    pub detected_context_tokens: Option<u32>,
    /// Declared by an admin.
    pub declared_context_tokens: Option<u32>,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            detected_tool_calling: ToolCalling::Unknown,
            declared_tool_calling: None,
            detected_context_tokens: None,
            declared_context_tokens: None,
        }
    }
}

/// Something wrong with a model for a job that needs tool calling and context (BR-15.2, BR-14.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuitabilityWarning {
    /// Tool calling was tested and is not supported.
    ToolCallingUnsupported,
    /// Tool calling has not been shown to work (untested or inconclusive).
    ToolCallingUnknown,
    /// The context is smaller than a job needs.
    ContextTooSmall {
        /// What the model has.
        have: u32,
        /// What a job needs.
        need: u32,
    },
    /// The context length is not known.
    ContextUnknown,
}

impl ModelCapabilities {
    /// Tool calling as used: declared if declared, otherwise detected.
    pub fn tool_calling(&self) -> ToolCalling {
        self.declared_tool_calling
            .unwrap_or(self.detected_tool_calling)
    }

    /// Context length as used: declared if declared, otherwise detected.
    pub fn context_tokens(&self) -> Option<u32> {
        self.declared_context_tokens
            .or(self.detected_context_tokens)
    }

    /// Record what a test found. Declared values are untouched (BR-14.5).
    pub fn apply_detection(&mut self, tool_calling: ToolCalling, context_tokens: Option<u32>) {
        self.detected_tool_calling = tool_calling;
        if context_tokens.is_some() {
            self.detected_context_tokens = context_tokens;
        }
    }

    /// Declare a context length.
    ///
    /// # Errors
    /// [`ConfigRuleError::ContextOutOfRange`] outside 1 to [`MAX_CONTEXT_TOKENS`].
    pub fn declare_context_tokens(&mut self, tokens: u32) -> Result<(), ConfigRuleError> {
        if !(1..=MAX_CONTEXT_TOKENS).contains(&tokens) {
            return Err(ConfigRuleError::ContextOutOfRange {
                max: MAX_CONTEXT_TOKENS,
            });
        }
        self.declared_context_tokens = Some(tokens);
        Ok(())
    }

    /// Whether a declared value disagrees with a known detected one (shown to the admin).
    pub fn disagrees_with_detection(&self) -> bool {
        let tools = self.declared_tool_calling.is_some_and(|declared| {
            self.detected_tool_calling != ToolCalling::Unknown
                && declared != self.detected_tool_calling
        });
        let context = matches!(
            (self.declared_context_tokens, self.detected_context_tokens),
            (Some(declared), Some(detected)) if declared != detected
        );
        tools || context
    }

    /// The reasons this model is not suitable for the orchestrator or an agent. Empty means
    /// suitable: tool calling supported and at least [`MIN_JOB_CONTEXT_TOKENS`] of context (A7).
    pub fn suitability_warnings(&self) -> Vec<SuitabilityWarning> {
        let mut warnings = Vec::new();
        match self.tool_calling() {
            ToolCalling::Supported => {}
            ToolCalling::Unsupported => warnings.push(SuitabilityWarning::ToolCallingUnsupported),
            ToolCalling::Unknown => warnings.push(SuitabilityWarning::ToolCallingUnknown),
        }
        match self.context_tokens() {
            Some(have) if have >= MIN_JOB_CONTEXT_TOKENS => {}
            Some(have) => warnings.push(SuitabilityWarning::ContextTooSmall {
                have,
                need: MIN_JOB_CONTEXT_TOKENS,
            }),
            None => warnings.push(SuitabilityWarning::ContextUnknown),
        }
        warnings
    }
}

/// A model listed under a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModel {
    /// Identifier (defaults point here, so renaming a model name keeps them).
    pub id: ProviderModelId,
    /// Owning provider.
    pub provider_id: ProviderId,
    /// The name the provider's API expects.
    pub name: ModelName,
    /// Optional friendly label.
    pub label: Option<String>,
    /// What it can do.
    pub capabilities: ModelCapabilities,
}

impl ProviderModel {
    /// Validate a friendly label: at most [`MAX_MODEL_LABEL_CHARS`], no control characters; empty
    /// means none.
    ///
    /// # Errors
    /// [`ConfigRuleError`] for a long or control-character label.
    pub fn clean_label(text: &str) -> Result<Option<String>, ConfigRuleError> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        clean_text("The label", text, MAX_MODEL_LABEL_CHARS).map(Some)
    }
}

/// Which default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultRole {
    /// The orchestrator.
    Orchestrator,
    /// Thread and task agents.
    Agents,
}

/// The default models (BR-15.1). A pointer is cleared when its model goes away (BR-15.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModelDefaults {
    /// The orchestrator's model.
    pub orchestrator: Option<ProviderModelId>,
    /// The model for thread and task agents.
    pub agents: Option<ProviderModelId>,
}

impl ModelDefaults {
    /// Clear every default that points at one of `removed`; returns which were cleared.
    pub fn clear_models(&mut self, removed: &[ProviderModelId]) -> Vec<DefaultRole> {
        let mut cleared = Vec::new();
        if self.orchestrator.is_some_and(|id| removed.contains(&id)) {
            self.orchestrator = None;
            cleared.push(DefaultRole::Orchestrator);
        }
        if self.agents.is_some_and(|id| removed.contains(&id)) {
            self.agents = None;
            cleared.push(DefaultRole::Agents);
        }
        cleared
    }

    /// The model chosen for a role.
    pub fn get(&self, role: DefaultRole) -> Option<ProviderModelId> {
        match role {
            DefaultRole::Orchestrator => self.orchestrator,
            DefaultRole::Agents => self.agents,
        }
    }
}
