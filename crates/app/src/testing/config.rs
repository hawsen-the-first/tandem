//! In-memory fakes for the U2 configuration repositories, the per-target lock and scripted probes.

use super::State;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use tandem_core::config::{
    Connector, ConnectorKind, ModelDefaults, ProbeResult, ProbeSubject, Provider, ProviderModel,
    SearchConfig, SearchKind, TestRun, TestStatus, TestTarget, ToolCalling,
};
use tandem_core::ids::{ProviderId, ProviderModelId, TestRunId, UserId};
use tandem_core::sandbox_policy::{PolicyScope, SandboxPolicy};
use tandem_ports::{
    ConnectorConnection, ConnectorProbe, ConnectorRepo, DefaultsRepo, LockGuard, Locks,
    ModelToProbe, PolicyRepo, ProviderConnection, ProviderProbe, ProviderRepo, ResultSink,
    RuntimeProbe, SearchConnection, SearchProbe, SearchRepo, SecretChange, StoreError, TestRunRepo,
};
use time::OffsetDateTime;

/// A provider with its sealed key, as the store holds it.
#[derive(Debug, Clone)]
pub struct StoredProvider {
    /// The provider.
    pub provider: Provider,
    /// The sealed key.
    pub sealed_key: Option<Vec<u8>>,
}

/// A search config with its sealed key.
#[derive(Debug, Clone)]
pub struct StoredSearch {
    /// The settings.
    pub config: SearchConfig,
    /// The sealed key.
    pub sealed_key: Option<Vec<u8>>,
}

/// Everything U2 stores.
#[derive(Debug, Clone, Default)]
pub struct ConfigState {
    /// Providers.
    pub providers: Vec<StoredProvider>,
    /// Models of all providers, in order.
    pub models: Vec<ProviderModel>,
    /// Default models.
    pub defaults: ModelDefaults,
    /// Search settings.
    pub search: Vec<StoredSearch>,
    /// The active search kind.
    pub search_active: Option<SearchKind>,
    /// Connectors with their sealed secret.
    pub connectors: Vec<(Connector, Vec<u8>)>,
    /// Saved policies.
    pub policies: Vec<SandboxPolicy>,
    /// Test runs.
    pub test_runs: Vec<TestRun>,
}

fn tool_calling_of(model: &ProviderModel) -> ToolCalling {
    model.capabilities.detected_tool_calling
}

#[async_trait]
impl ProviderRepo for State {
    async fn insert(
        &mut self,
        provider: &Provider,
        sealed_key: Option<&[u8]>,
    ) -> Result<(), StoreError> {
        let key = provider.name.unique_key();
        if self
            .config
            .providers
            .iter()
            .any(|stored| stored.provider.name.unique_key() == key)
        {
            return Err(StoreError::Conflict("providers.name"));
        }
        self.config.providers.push(StoredProvider {
            provider: provider.clone(),
            sealed_key: sealed_key.map(<[u8]>::to_vec),
        });
        Ok(())
    }

    async fn update(&mut self, provider: &Provider, key: SecretChange) -> Result<(), StoreError> {
        let unique = provider.name.unique_key();
        if self.config.providers.iter().any(|stored| {
            stored.provider.id != provider.id && stored.provider.name.unique_key() == unique
        }) {
            return Err(StoreError::Conflict("providers.name"));
        }
        let stored = self
            .config
            .providers
            .iter_mut()
            .find(|stored| stored.provider.id == provider.id)
            .ok_or(StoreError::NotFound)?;
        stored.provider = provider.clone();
        match key {
            SecretChange::Keep => {}
            SecretChange::Set(bytes) => stored.sealed_key = Some(bytes),
            SecretChange::Clear => stored.sealed_key = None,
        }
        Ok(())
    }

    async fn get(&mut self, id: ProviderId) -> Result<Option<Provider>, StoreError> {
        Ok(self
            .config
            .providers
            .iter()
            .find(|stored| stored.provider.id == id)
            .map(|stored| stored.provider.clone()))
    }

    async fn list(&mut self) -> Result<Vec<Provider>, StoreError> {
        let mut providers: Vec<Provider> = self
            .config
            .providers
            .iter()
            .map(|stored| stored.provider.clone())
            .collect();
        providers.sort_by_key(|provider| provider.name.unique_key());
        Ok(providers)
    }

    async fn sealed_key(&mut self, id: ProviderId) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .config
            .providers
            .iter()
            .find(|stored| stored.provider.id == id)
            .and_then(|stored| stored.sealed_key.clone()))
    }

    async fn delete(&mut self, id: ProviderId) -> Result<(), StoreError> {
        let before = self.config.providers.len();
        self.config
            .providers
            .retain(|stored| stored.provider.id != id);
        if self.config.providers.len() == before {
            return Err(StoreError::NotFound);
        }
        let removed: Vec<ProviderModelId> = self
            .config
            .models
            .iter()
            .filter(|model| model.provider_id == id)
            .map(|model| model.id)
            .collect();
        self.config.models.retain(|model| model.provider_id != id);
        self.config.defaults.clear_models(&removed);
        Ok(())
    }

    async fn models(&mut self, provider_id: ProviderId) -> Result<Vec<ProviderModel>, StoreError> {
        Ok(self
            .config
            .models
            .iter()
            .filter(|model| model.provider_id == provider_id)
            .cloned()
            .collect())
    }

    async fn all_models(&mut self) -> Result<Vec<ProviderModel>, StoreError> {
        Ok(self.config.models.clone())
    }

    async fn get_model(
        &mut self,
        id: ProviderModelId,
    ) -> Result<Option<ProviderModel>, StoreError> {
        Ok(self
            .config
            .models
            .iter()
            .find(|model| model.id == id)
            .cloned())
    }

    async fn replace_models(
        &mut self,
        provider_id: ProviderId,
        models: &[ProviderModel],
    ) -> Result<Vec<ProviderModelId>, StoreError> {
        let mut names = HashSet::new();
        if !models.iter().all(|model| names.insert(model.name.as_str())) {
            return Err(StoreError::Conflict("provider_models.name"));
        }
        let removed: Vec<ProviderModelId> = self
            .config
            .models
            .iter()
            .filter(|model| {
                model.provider_id == provider_id && !models.iter().any(|kept| kept.id == model.id)
            })
            .map(|model| model.id)
            .collect();
        let previous: HashMap<ProviderModelId, ProviderModel> = self
            .config
            .models
            .iter()
            .filter(|model| model.provider_id == provider_id)
            .map(|model| (model.id, model.clone()))
            .collect();
        self.config
            .models
            .retain(|model| model.provider_id != provider_id);
        for model in models {
            let mut stored = model.clone();
            if let Some(old) = previous.get(&model.id) {
                // Detected values belong to tests, not to this write.
                stored.capabilities.detected_tool_calling = tool_calling_of(old);
                stored.capabilities.detected_context_tokens =
                    old.capabilities.detected_context_tokens;
            }
            self.config.models.push(stored);
        }
        Ok(removed)
    }

    async fn set_detected(
        &mut self,
        id: ProviderModelId,
        tool_calling: ToolCalling,
        context_tokens: Option<u32>,
    ) -> Result<(), StoreError> {
        if let Some(model) = self.config.models.iter_mut().find(|model| model.id == id) {
            model
                .capabilities
                .apply_detection(tool_calling, context_tokens);
        }
        Ok(())
    }
}

#[async_trait]
impl DefaultsRepo for State {
    async fn get(&mut self) -> Result<ModelDefaults, StoreError> {
        Ok(self.config.defaults)
    }

    async fn save(
        &mut self,
        defaults: &ModelDefaults,
        _by: UserId,
        _now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        self.config.defaults = *defaults;
        Ok(())
    }
}

#[async_trait]
impl SearchRepo for State {
    async fn list(&mut self) -> Result<Vec<SearchConfig>, StoreError> {
        Ok(self
            .config
            .search
            .iter()
            .map(|stored| stored.config.clone())
            .collect())
    }

    async fn get(&mut self, kind: SearchKind) -> Result<Option<SearchConfig>, StoreError> {
        Ok(self
            .config
            .search
            .iter()
            .find(|stored| stored.config.kind == kind)
            .map(|stored| stored.config.clone()))
    }

    async fn save(&mut self, config: &SearchConfig, key: SecretChange) -> Result<(), StoreError> {
        match self
            .config
            .search
            .iter_mut()
            .find(|stored| stored.config.kind == config.kind)
        {
            Some(stored) => {
                stored.config = config.clone();
                match key {
                    SecretChange::Keep => {}
                    SecretChange::Set(bytes) => stored.sealed_key = Some(bytes),
                    SecretChange::Clear => stored.sealed_key = None,
                }
            }
            None => self.config.search.push(StoredSearch {
                config: config.clone(),
                sealed_key: match key {
                    SecretChange::Set(bytes) => Some(bytes),
                    SecretChange::Keep | SecretChange::Clear => None,
                },
            }),
        }
        Ok(())
    }

    async fn sealed_key(&mut self, kind: SearchKind) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .config
            .search
            .iter()
            .find(|stored| stored.config.kind == kind)
            .and_then(|stored| stored.sealed_key.clone()))
    }

    async fn active(&mut self) -> Result<Option<SearchKind>, StoreError> {
        Ok(self.config.search_active)
    }

    async fn set_active(
        &mut self,
        kind: Option<SearchKind>,
        _by: UserId,
        _now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        if let Some(kind) = kind
            && !self
                .config
                .search
                .iter()
                .any(|stored| stored.config.kind == kind)
        {
            return Err(StoreError::NotFound);
        }
        self.config.search_active = kind;
        Ok(())
    }
}

#[async_trait]
impl ConnectorRepo for State {
    async fn get(&mut self, kind: ConnectorKind) -> Result<Option<Connector>, StoreError> {
        Ok(self
            .config
            .connectors
            .iter()
            .find(|(connector, _)| connector.kind == kind)
            .map(|(connector, _)| connector.clone()))
    }

    async fn save(
        &mut self,
        connector: &Connector,
        sealed_secret: &[u8],
    ) -> Result<(), StoreError> {
        let mut saved = connector.clone();
        saved.account_login = None;
        saved.token_expires_at = None;
        self.config
            .connectors
            .retain(|(existing, _)| existing.kind != connector.kind);
        self.config.connectors.push((saved, sealed_secret.to_vec()));
        Ok(())
    }

    async fn sealed_secret(&mut self, kind: ConnectorKind) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .config
            .connectors
            .iter()
            .find(|(connector, _)| connector.kind == kind)
            .map(|(_, sealed)| sealed.clone()))
    }

    async fn set_identity(
        &mut self,
        kind: ConnectorKind,
        account_login: Option<&str>,
        token_expires_at: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        if let Some((connector, _)) = self
            .config
            .connectors
            .iter_mut()
            .find(|(connector, _)| connector.kind == kind)
        {
            connector.account_login = account_login.map(str::to_owned);
            connector.token_expires_at = token_expires_at;
        }
        Ok(())
    }

    async fn delete(&mut self, kind: ConnectorKind) -> Result<(), StoreError> {
        let before = self.config.connectors.len();
        self.config
            .connectors
            .retain(|(connector, _)| connector.kind != kind);
        if self.config.connectors.len() == before {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

#[async_trait]
impl PolicyRepo for State {
    async fn get(&mut self, scope: PolicyScope) -> Result<Option<SandboxPolicy>, StoreError> {
        Ok(self
            .config
            .policies
            .iter()
            .find(|policy| policy.scope == scope)
            .cloned())
    }

    async fn save(
        &mut self,
        policy: &SandboxPolicy,
        previous_version: u32,
        _by: UserId,
        _now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        match self
            .config
            .policies
            .iter_mut()
            .find(|stored| stored.scope == policy.scope)
        {
            Some(stored) if stored.version == previous_version => *stored = policy.clone(),
            Some(_) => return Err(StoreError::Conflict("sandbox_policies.version")),
            None if previous_version == 1 => self.config.policies.push(policy.clone()),
            None => return Err(StoreError::Conflict("sandbox_policies.version")),
        }
        Ok(())
    }
}

#[async_trait]
impl TestRunRepo for State {
    async fn insert(&mut self, run: &TestRun) -> Result<(), StoreError> {
        if run.is_active()
            && self
                .config
                .test_runs
                .iter()
                .any(|existing| existing.target == run.target && existing.is_active())
        {
            return Err(StoreError::Conflict("config_test_runs.active"));
        }
        self.config.test_runs.push(run.clone());
        Ok(())
    }

    async fn get(&mut self, id: TestRunId) -> Result<Option<TestRun>, StoreError> {
        Ok(self
            .config
            .test_runs
            .iter()
            .find(|run| run.id == id)
            .cloned())
    }

    async fn save(&mut self, run: &TestRun) -> Result<(), StoreError> {
        let stored = self
            .config
            .test_runs
            .iter_mut()
            .find(|existing| existing.id == run.id)
            .ok_or(StoreError::NotFound)?;
        *stored = run.clone();
        Ok(())
    }

    async fn active_for(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError> {
        Ok(self
            .config
            .test_runs
            .iter()
            .find(|run| run.target == target && run.is_active())
            .cloned())
    }

    async fn latest_finished(&mut self, target: TestTarget) -> Result<Option<TestRun>, StoreError> {
        Ok(self
            .config
            .test_runs
            .iter()
            .filter(|run| run.target == target && !run.is_active())
            .max_by_key(|run| run.requested_at)
            .cloned())
    }

    async fn queued_before(&mut self, cutoff: OffsetDateTime) -> Result<Vec<TestRun>, StoreError> {
        Ok(self
            .config
            .test_runs
            .iter()
            .filter(|run| {
                run.status == tandem_core::config::RunStatus::Queued && run.requested_at < cutoff
            })
            .cloned()
            .collect())
    }

    async fn prune(
        &mut self,
        keep_per_target: u32,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StoreError> {
        let mut finished: Vec<&TestRun> = self
            .config
            .test_runs
            .iter()
            .filter(|run| !run.is_active())
            .collect();
        finished.sort_by_key(|run| std::cmp::Reverse(run.requested_at));
        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut doomed = Vec::new();
        for run in finished {
            let position = seen.entry(run.target.key()).or_insert(0);
            *position += 1;
            if *position > 1 && (*position > keep_per_target || run.requested_at < cutoff) {
                doomed.push(run.id);
            }
        }
        let before = self.config.test_runs.len();
        self.config
            .test_runs
            .retain(|run| !doomed.contains(&run.id));
        Ok(u64::try_from(before - self.config.test_runs.len()).unwrap_or(u64::MAX))
    }
}

/// A per-key lock held in memory, shared by every worker in a test.
#[derive(Debug, Clone, Default)]
pub struct MemoryLocks {
    held: Arc<Mutex<HashSet<String>>>,
}

impl MemoryLocks {
    /// Take a key as if another worker held it; returns a guard that frees it when released.
    ///
    /// # Panics
    /// If the key is already held (a test bug).
    #[allow(clippy::panic)]
    pub fn hold(&self, key: &str) -> MemoryLockGuard {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(held.insert(key.to_owned()), "{key} is already held");
        MemoryLockGuard {
            held: Arc::clone(&self.held),
            key: key.to_owned(),
        }
    }

    /// Whether a key is held now.
    pub fn is_held(&self, key: &str) -> bool {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(key)
    }
}

/// Frees its key when released or dropped.
#[derive(Debug)]
pub struct MemoryLockGuard {
    held: Arc<Mutex<HashSet<String>>>,
    key: String,
}

impl Drop for MemoryLockGuard {
    fn drop(&mut self) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

#[async_trait]
impl LockGuard for MemoryLockGuard {
    async fn release(self: Box<Self>) {}
}

#[async_trait]
impl Locks for MemoryLocks {
    async fn try_lock(&self, key: &str) -> Result<Option<Box<dyn LockGuard>>, StoreError> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if !held.insert(key.to_owned()) {
            return Ok(None);
        }
        Ok(Some(Box::new(MemoryLockGuard {
            held: Arc::clone(&self.held),
            key: key.to_owned(),
        })))
    }
}

/// What a scripted probe was asked, so a test can check secrets arrived and nothing else leaked.
#[derive(Debug, Clone, Default)]
pub struct ProbeCalls {
    /// The opened provider keys the provider probe saw.
    pub provider_keys: Vec<Option<String>>,
    /// How many times each probe ran.
    pub provider_runs: usize,
    /// Search probe runs.
    pub search_runs: usize,
    /// Connector probe runs.
    pub connector_runs: usize,
    /// Runtime probe runs.
    pub runtime_runs: usize,
}

/// Probes whose answers a test sets; they record what they were given.
#[derive(Debug, Clone)]
pub struct ScriptedProbes {
    /// What each probe saw.
    pub calls: Arc<Mutex<ProbeCalls>>,
    /// The status every model gets (the provider gets `Ok`).
    pub model_status: Arc<Mutex<TestStatus>>,
    /// The detected tool calling for models that pass.
    pub tool_calling: Arc<Mutex<ToolCalling>>,
    /// The detected context length for models that pass.
    pub context_tokens: Arc<Mutex<Option<u32>>>,
    /// The result for the search probe.
    pub search_status: Arc<Mutex<TestStatus>>,
    /// The result for the connector probe.
    pub connector_result: Arc<Mutex<ProbeResult>>,
    /// The result for the runtime probe.
    pub runtime_status: Arc<Mutex<TestStatus>>,
    /// When set, the provider probe fails the test of its models after the first.
    pub delay: Arc<Mutex<Option<std::time::Duration>>>,
}

impl Default for ScriptedProbes {
    fn default() -> Self {
        Self {
            calls: Arc::default(),
            model_status: Arc::new(Mutex::new(TestStatus::Ok)),
            tool_calling: Arc::new(Mutex::new(ToolCalling::Supported)),
            context_tokens: Arc::new(Mutex::new(Some(32_768))),
            search_status: Arc::new(Mutex::new(TestStatus::Ok)),
            connector_result: Arc::new(Mutex::new(ProbeResult::new(
                ProbeSubject::Connector,
                TestStatus::Ok,
            ))),
            runtime_status: Arc::new(Mutex::new(TestStatus::Ok)),
            delay: Arc::default(),
        }
    }
}

fn read<T: Clone>(slot: &Mutex<T>) -> T {
    slot.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[async_trait]
impl ProviderProbe for ScriptedProbes {
    async fn probe(
        &self,
        connection: &ProviderConnection,
        models: &[ModelToProbe],
        sink: &dyn ResultSink,
    ) {
        {
            let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
            calls.provider_runs += 1;
            calls.provider_keys.push(
                connection
                    .api_key
                    .as_ref()
                    .map(|key| key.expose().to_owned()),
            );
        }
        if let Some(delay) = read(&self.delay) {
            tokio::time::sleep(delay).await;
        }
        sink.record(ProbeResult::new(ProbeSubject::Provider, TestStatus::Ok))
            .await;
        for model in models {
            let status = read(&self.model_status);
            let mut result = ProbeResult::new(ProbeSubject::Model(model.id), status);
            if status == TestStatus::Ok {
                result.details.tool_calling = Some(read(&self.tool_calling));
                result.details.context_tokens = read(&self.context_tokens);
            }
            sink.record(result).await;
        }
    }
}

#[async_trait]
impl SearchProbe for ScriptedProbes {
    async fn test(&self, _connection: &SearchConnection) -> ProbeResult {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .search_runs += 1;
        ProbeResult::new(ProbeSubject::Search, read(&self.search_status))
    }
}

#[async_trait]
impl ConnectorProbe for ScriptedProbes {
    async fn test(&self, _connection: &ConnectorConnection) -> ProbeResult {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connector_runs += 1;
        read(&self.connector_result)
    }
}

#[async_trait]
impl RuntimeProbe for ScriptedProbes {
    async fn probe(&self) -> ProbeResult {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .runtime_runs += 1;
        ProbeResult::new(ProbeSubject::Runtime, read(&self.runtime_status))
    }
}
