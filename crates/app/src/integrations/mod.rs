//! Instance administration (U2): providers, default models, search, the GitHub connector, the
//! sandbox policy and their tests (W16 to W24). Every method authorises as an admin first (BR-21.3),
//! changes and audits in one transaction (BR-21.1), and never returns or logs a secret (BR-20).

mod defaults;
mod policy;
mod providers;
mod read;
mod runner;
mod search;
mod test_runs;

pub use defaults::{DefaultChoice, DefaultsInput, DefaultsView};
pub use providers::{ModelInput, ModelView, ProviderInput, ProviderView, RemovalImpact};
pub use read::{ConfigCache, ListedModel, ResolvedModel};
pub use search::{ConnectorView, SearchActivation, SearchInput, SearchOverview, SearchView};

use crate::error::FieldIssue;
use tandem_core::config::DefaultRole;
use tandem_support::Secret;

/// What a form says about a secret field (BR-20.3): empty means keep, a value replaces, and
/// "remove" is its own explicit action.
#[derive(Clone)]
pub enum SecretInput {
    /// Leave the stored secret as it is.
    Keep,
    /// Replace it.
    Set(Secret),
    /// Delete it.
    Remove,
}

impl std::fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Keep => "SecretInput::Keep",
            Self::Set(_) => "SecretInput::Set(redacted)",
            Self::Remove => "SecretInput::Remove",
        })
    }
}

impl SecretInput {
    /// Build from the usual form fields: a remove flag and the text typed, where empty text means
    /// "unchanged".
    pub fn from_form(text: Option<&str>, remove: bool) -> Self {
        if remove {
            return Self::Remove;
        }
        match text {
            Some(value) if !value.trim().is_empty() => Self::Set(Secret::new(value.trim())),
            _ => Self::Keep,
        }
    }
}

/// Vault purposes, one per kind of secret; each value is also bound to its row (BR-20.1).
pub(crate) const PROVIDER_KEY_PURPOSE: &str = "provider_api_key";
pub(crate) const SEARCH_KEY_PURPOSE: &str = "search_api_key";
pub(crate) const CONNECTOR_SECRET_PURPOSE: &str = "connector_secret";

/// The longest secret accepted: far beyond any real key, short enough to bound storage.
const MAX_SECRET_CHARS: usize = 4096;

pub(crate) fn check_secret(field: &'static str, input: &SecretInput, issues: &mut Vec<FieldIssue>) {
    if let SecretInput::Set(secret) = input {
        let text = secret.expose();
        if text.chars().count() > MAX_SECRET_CHARS {
            issues.push(FieldIssue {
                field,
                message: "That is too long to be a key".to_owned(),
            });
        } else if text.chars().any(char::is_control) {
            issues.push(FieldIssue {
                field,
                message: "A key cannot contain control characters".to_owned(),
            });
        }
    }
}

pub(crate) fn role_name(role: DefaultRole) -> &'static str {
    match role {
        DefaultRole::Orchestrator => "orchestrator",
        DefaultRole::Agents => "agents",
    }
}

pub(crate) fn set_text(saved: bool) -> &'static str {
    if saved { "set" } else { "not set" }
}

pub(crate) fn roles_text(roles: &[DefaultRole]) -> String {
    roles
        .iter()
        .map(|role| role_name(*role))
        .collect::<Vec<_>>()
        .join(",")
}
