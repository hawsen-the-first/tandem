//! Repositories for instance configuration (U2). Secrets cross this boundary already sealed by the
//! vault: the application seals and opens them, the store only keeps the bytes (SE-20).

use crate::error::StoreError;
use async_trait::async_trait;
use tandem_core::config::{
    Connector, ConnectorKind, ModelCapabilities, ModelDefaults, Provider, ProviderModel,
    SearchConfig, SearchKind, TestRun, TestTarget, ToolCalling,
};
use tandem_core::ids::{ProviderId, ProviderModelId, TestRunId, UserId};
use tandem_core::sandbox_policy::{PolicyScope, SandboxPolicy};
use time::OffsetDateTime;

/// What to do with a stored sealed secret when its owner is saved (BR-20.3).
#[derive(Clone, PartialEq, Eq)]
pub enum SecretChange {
    /// Leave it as it is (an empty field on an edit).
    Keep,
    /// Replace it with this sealed value.
    Set(Vec<u8>),
    /// Remove it (the explicit "remove" action).
    Clear,
}

impl std::fmt::Debug for SecretChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Keep => "SecretChange::Keep",
            Self::Set(_) => "SecretChange::Set(sealed)",
            Self::Clear => "SecretChange::Clear",
        })
    }
}

/// Providers and their models.
#[async_trait]
pub trait ProviderRepo: Send {
    /// Insert a provider with an optional sealed key.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] (`providers.name`) when another provider has the name, ignoring case.
    async fn insert(
        &mut self,
        provider: &Provider,
        sealed_key: Option<&[u8]>,
    ) -> Result<(), StoreError>;
    /// Save changes to a provider's name, URL and audit fields, and apply the key change.
    /// `key_saved_at` on the provider is what gets stored.
    ///
    /// # Errors
    /// [`StoreError::NotFound`]; [`StoreError::Conflict`] (`providers.name`).
    async fn update(&mut self, provider: &Provider, key: SecretChange) -> Result<(), StoreError>;
    /// Fetch a provider.
    async fn get(&mut self, id: ProviderId) -> Result<Option<Provider>, StoreError>;
    /// Every provider, by name.
    async fn list(&mut self) -> Result<Vec<Provider>, StoreError>;
    /// The sealed key, for the one place that opens it.
    async fn sealed_key(&mut self, id: ProviderId) -> Result<Option<Vec<u8>>, StoreError>;
    /// Delete a provider and its models.
    ///
    /// # Errors
    /// [`StoreError::NotFound`].
    async fn delete(&mut self, id: ProviderId) -> Result<(), StoreError>;

    /// A provider's models in display order.
    async fn models(&mut self, provider_id: ProviderId) -> Result<Vec<ProviderModel>, StoreError>;
    /// Every model of every provider, by provider then position.
    async fn all_models(&mut self) -> Result<Vec<ProviderModel>, StoreError>;
    /// Fetch one model.
    async fn get_model(&mut self, id: ProviderModelId)
    -> Result<Option<ProviderModel>, StoreError>;
    /// Make the provider's models exactly `models` (matched by id): new ones are added, kept ones
    /// have their name, label, order and **declared** values updated (detected values are left
    /// alone), others are removed. Returns the removed ids.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] (`provider_models.name`) for a duplicate name.
    async fn replace_models(
        &mut self,
        provider_id: ProviderId,
        models: &[ProviderModel],
    ) -> Result<Vec<ProviderModelId>, StoreError>;
    /// Record what a test found for a model. Declared values are untouched (BR-14.5). A missing
    /// model (removed while the test ran) is ignored.
    async fn set_detected(
        &mut self,
        id: ProviderModelId,
        tool_calling: ToolCalling,
        context_tokens: Option<u32>,
    ) -> Result<(), StoreError>;
}

/// The two default models.
#[async_trait]
pub trait DefaultsRepo: Send {
    /// The defaults; none set when nothing was ever saved.
    async fn get(&mut self) -> Result<ModelDefaults, StoreError>;
    /// Save the defaults.
    async fn save(
        &mut self,
        defaults: &ModelDefaults,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
}

/// Search settings per kind and the single active kind.
#[async_trait]
pub trait SearchRepo: Send {
    /// Every kind that has saved settings.
    async fn list(&mut self) -> Result<Vec<SearchConfig>, StoreError>;
    /// One kind's settings.
    async fn get(&mut self, kind: SearchKind) -> Result<Option<SearchConfig>, StoreError>;
    /// Insert or update one kind's settings and apply the key change.
    async fn save(&mut self, config: &SearchConfig, key: SecretChange) -> Result<(), StoreError>;
    /// The sealed key of a kind.
    async fn sealed_key(&mut self, kind: SearchKind) -> Result<Option<Vec<u8>>, StoreError>;
    /// The active kind, if any.
    async fn active(&mut self) -> Result<Option<SearchKind>, StoreError>;
    /// Make a kind active (it must have saved settings) or none.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] when the kind has no saved settings.
    async fn set_active(
        &mut self,
        kind: Option<SearchKind>,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
}

/// Connectors (GitHub).
#[async_trait]
pub trait ConnectorRepo: Send {
    /// A connector, if configured.
    async fn get(&mut self, kind: ConnectorKind) -> Result<Option<Connector>, StoreError>;
    /// Insert or replace a connector with its sealed secret. A replaced token drops the identity
    /// facts of the old one (they are filled again by the next test).
    async fn save(&mut self, connector: &Connector, sealed_secret: &[u8])
    -> Result<(), StoreError>;
    /// The sealed secret.
    async fn sealed_secret(&mut self, kind: ConnectorKind) -> Result<Option<Vec<u8>>, StoreError>;
    /// Record the account and expiry a test found.
    async fn set_identity(
        &mut self,
        kind: ConnectorKind,
        account_login: Option<&str>,
        token_expires_at: Option<OffsetDateTime>,
    ) -> Result<(), StoreError>;
    /// Remove a connector and its secret.
    ///
    /// # Errors
    /// [`StoreError::NotFound`].
    async fn delete(&mut self, kind: ConnectorKind) -> Result<(), StoreError>;
}

/// Sandbox policies.
#[async_trait]
pub trait PolicyRepo: Send {
    /// A stored policy; `None` means the factory default applies.
    async fn get(&mut self, scope: PolicyScope) -> Result<Option<SandboxPolicy>, StoreError>;
    /// Save a policy whose `version` is one more than `previous_version`, the version the caller
    /// read (1 for the unsaved factory default).
    ///
    /// # Errors
    /// [`StoreError::Conflict`] (`sandbox_policies.version`) when someone saved in between.
    async fn save(
        &mut self,
        policy: &SandboxPolicy,
        previous_version: u32,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError>;
}

/// Configuration test runs.
#[async_trait]
pub trait TestRunRepo: Send {
    /// Insert a queued run.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] (`config_test_runs.active`) when the target already has a queued
    /// or running run.
    async fn insert(&mut self, run: &TestRun) -> Result<(), StoreError>;
    /// Fetch a run.
    async fn get(&mut self, id: TestRunId) -> Result<Option<TestRun>, StoreError>;
    /// Save a run's state, timings and results.
    async fn save(&mut self, run: &TestRun) -> Result<(), StoreError>;
    /// The queued or running run for a target.
    async fn active_for(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError>;
    /// The newest run for a target that finished (succeeded or failed).
    async fn latest_finished(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError>;
    /// Runs still queued since before `cutoff` (nobody picked them up).
    async fn queued_before(&mut self, cutoff: OffsetDateTime) -> Result<Vec<TestRun>, StoreError>;
    /// Delete finished runs, keeping each target's newest finished run always and otherwise the
    /// newest `keep_per_target` runs requested since `cutoff`. Returns how many were deleted.
    async fn prune(
        &mut self,
        keep_per_target: u32,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StoreError>;
}

/// Held while a run executes; releasing it (or dropping it) lets another worker take the target.
#[async_trait]
pub trait LockGuard: Send {
    /// Release the lock.
    async fn release(self: Box<Self>);
}

/// Session-level locks held across transactions (the advisory lock per target, pattern 2).
#[async_trait]
pub trait Locks: Send + Sync {
    /// Take the lock for `key` without waiting; `None` when another holder has it.
    async fn try_lock(&self, key: &str) -> Result<Option<Box<dyn LockGuard>>, StoreError>;
}

/// Convenience for building [`ModelCapabilities`] from stored columns.
pub fn capabilities_from(
    detected_tool_calling: ToolCalling,
    declared_tool_calling: Option<ToolCalling>,
    detected_context_tokens: Option<u32>,
    declared_context_tokens: Option<u32>,
) -> ModelCapabilities {
    ModelCapabilities {
        detected_tool_calling,
        declared_tool_calling,
        detected_context_tokens,
        declared_context_tokens,
    }
}
