//! Instance configuration entities and rules (U2: BR-14 to BR-17, BR-20): providers and their
//! models, default models, search, the GitHub connector and test runs. Pure; storage, sealing and
//! network calls live elsewhere. Secrets are never part of these entities: they are sealed in the
//! store, and the entities only record that one is saved and when.

mod connectors;
mod provider;
mod test_run;

pub use connectors::{
    Connector, ConnectorAuth, ConnectorKind, DEFAULT_SEARXNG_URL, SearchConfig, SearchKind,
};
pub use provider::{
    DefaultRole, MAX_CONTEXT_TOKENS, MAX_MODEL_LABEL_CHARS, MAX_MODEL_NAME_CHARS,
    MAX_MODELS_PER_PROVIDER, MAX_PROVIDER_NAME_CHARS, MAX_PROVIDERS, MIN_JOB_CONTEXT_TOKENS,
    ModelCapabilities, ModelDefaults, ModelName, Provider, ProviderKind, ProviderModel,
    ProviderName, SuitabilityWarning, ToolCalling, validate_model_names,
};
pub use test_run::{
    FailureReason, MAX_REPOSITORIES_LISTED, ProbeDetails, ProbeNote, ProbeResult, ProbeSubject,
    RunError, RunStatus, TestRun, TestStatus, TestTarget,
};

/// Why a configuration value was refused. The text is shown next to the field.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigRuleError {
    /// A required text was empty.
    #[error("{0} is required")]
    Empty(&'static str),
    /// A text was too long.
    #[error("{what} can be at most {max} characters")]
    TooLong {
        /// The field.
        what: &'static str,
        /// The limit.
        max: usize,
    },
    /// A text contains control characters.
    #[error("{0} cannot contain control characters")]
    ControlCharacters(&'static str),
    /// A name is used twice.
    #[error("That model is already listed")]
    DuplicateModel,
    /// Too many items.
    #[error("{what} are limited to {max}")]
    TooMany {
        /// The things being counted.
        what: &'static str,
        /// The limit.
        max: usize,
    },
    /// A number outside its range.
    #[error("Enter a whole number of tokens from 1 to {max}")]
    ContextOutOfRange {
        /// The largest accepted value.
        max: u32,
    },
}

/// Trim, check emptiness, length and control characters.
pub(crate) fn clean_text(
    what: &'static str,
    text: &str,
    max: usize,
) -> Result<String, ConfigRuleError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ConfigRuleError::Empty(what));
    }
    if text.chars().any(char::is_control) {
        return Err(ConfigRuleError::ControlCharacters(what));
    }
    if text.chars().count() > max {
        return Err(ConfigRuleError::TooLong { what, max });
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests;
