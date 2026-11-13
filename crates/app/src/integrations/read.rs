//! The configuration read port for the runtime (W24, LC-31): the server-side view of what admins
//! configured. Never exposed to the browser. Snapshots are cached for five seconds per process and
//! cleared by local writes (SC-6, PF-9); secrets stay sealed in the snapshot and are opened on
//! demand into values that cannot print themselves (D6).

use crate::deps::App;
use crate::error::AppError;
use std::sync::{Arc, Mutex, PoisonError};
use tandem_core::config::{
    Connector, ConnectorKind, ModelDefaults, Provider, ProviderModel, SearchConfig, SearchKind,
    SuitabilityWarning,
};
use tandem_core::ids::ProviderModelId;
use tandem_core::sandbox_policy::{PolicyScope, SandboxPolicy};
use tandem_ports::{ConnectorConnection, ProviderConnection, SearchConnection};
use tandem_support::Secret;
use time::{Duration, OffsetDateTime};

/// How long a snapshot may be reused (A12).
pub(crate) const SNAPSHOT_LIFETIME: Duration = Duration::seconds(5);

#[derive(Debug)]
pub(crate) struct ProviderRecord {
    pub(crate) provider: Provider,
    pub(crate) sealed_key: Option<Vec<u8>>,
    pub(crate) models: Vec<ProviderModel>,
}

#[derive(Debug)]
pub(crate) struct SearchRecord {
    pub(crate) config: SearchConfig,
    pub(crate) sealed_key: Option<Vec<u8>>,
}

/// Everything the runtime reads, as stored (secrets sealed).
#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) providers: Vec<ProviderRecord>,
    pub(crate) defaults: ModelDefaults,
    pub(crate) search: Vec<SearchRecord>,
    pub(crate) search_active: Option<SearchKind>,
    pub(crate) connector: Option<(Connector, Vec<u8>)>,
    pub(crate) policy: SandboxPolicy,
}

type CachedSnapshot = (OffsetDateTime, Arc<Snapshot>);

/// The per-process snapshot cache.
#[derive(Debug, Clone, Default)]
pub struct ConfigCache {
    slot: Arc<Mutex<Option<CachedSnapshot>>>,
}

impl ConfigCache {
    /// Forget the snapshot so the next read goes to the database.
    pub fn clear(&self) {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn fresh(&self, now: OffsetDateTime) -> Option<Arc<Snapshot>> {
        let slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        slot.as_ref()
            .filter(|(taken, _)| now - *taken < SNAPSHOT_LIFETIME && now >= *taken)
            .map(|(_, snapshot)| Arc::clone(snapshot))
    }

    fn store(&self, now: OffsetDateTime, snapshot: Arc<Snapshot>) {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some((now, snapshot));
    }
}

/// A model with the connection to reach it and what is wrong with it for a job (BR-14.8).
#[derive(Debug)]
pub struct ResolvedModel {
    /// The provider.
    pub provider: Provider,
    /// The model.
    pub model: ProviderModel,
    /// Where and how to call it. Holds the opened key in memory only.
    pub connection: ProviderConnection,
    /// Empty when suitable; the runtime refuses a job the model cannot do (BR-15.2).
    pub warnings: Vec<SuitabilityWarning>,
}

/// A listed model with its provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedModel {
    /// The provider's name.
    pub provider_name: String,
    /// The model.
    pub model: ProviderModel,
    /// Empty when suitable.
    pub warnings: Vec<SuitabilityWarning>,
}

impl App {
    /// Read everything from the database now.
    pub(crate) async fn load_snapshot(&self) -> Result<Arc<Snapshot>, AppError> {
        let mut tx = self.store.begin().await?;
        let mut providers = Vec::new();
        for provider in tx.providers().list().await? {
            let sealed_key = tx.providers().sealed_key(provider.id).await?;
            let models = tx.providers().models(provider.id).await?;
            providers.push(ProviderRecord {
                provider,
                sealed_key,
                models,
            });
        }
        let defaults = tx.defaults().get().await?;
        let mut search = Vec::new();
        for config in tx.search().list().await? {
            let sealed_key = tx.search().sealed_key(config.kind).await?;
            search.push(SearchRecord { config, sealed_key });
        }
        let search_active = tx.search().active().await?;
        let connector = match tx.connectors().get(ConnectorKind::Github).await? {
            Some(connector) => tx
                .connectors()
                .sealed_secret(ConnectorKind::Github)
                .await?
                .map(|sealed| (connector, sealed)),
            None => None,
        };
        let policy = tx
            .policies()
            .get(PolicyScope::Instance)
            .await?
            .unwrap_or_else(SandboxPolicy::factory_default);
        Ok(Arc::new(Snapshot {
            providers,
            defaults,
            search,
            search_active,
            connector,
            policy,
        }))
    }

    /// The cached snapshot, or a fresh one when it is older than five seconds.
    pub(crate) async fn snapshot(&self) -> Result<Arc<Snapshot>, AppError> {
        let now = self.clock.now();
        if let Some(snapshot) = self.config_cache.fresh(now) {
            return Ok(snapshot);
        }
        let snapshot = self.load_snapshot().await?;
        self.config_cache.store(now, Arc::clone(&snapshot));
        Ok(snapshot)
    }

    pub(crate) fn open_secret(
        &self,
        purpose: &str,
        row_id: &str,
        sealed: &[u8],
    ) -> Result<Secret, AppError> {
        let bytes = self.vault.open(purpose, row_id, sealed)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::Internal("a stored secret is not text".to_owned()))?;
        Ok(Secret::new(text))
    }

    pub(crate) fn provider_connection(
        &self,
        record: &ProviderRecord,
    ) -> Result<ProviderConnection, AppError> {
        let api_key = record
            .sealed_key
            .as_deref()
            .map(|sealed| {
                self.open_secret(
                    super::PROVIDER_KEY_PURPOSE,
                    &record.provider.id.to_string(),
                    sealed,
                )
            })
            .transpose()?;
        Ok(ProviderConnection {
            base_url: record.provider.base_url.clone(),
            api_key,
        })
    }

    pub(crate) fn search_connection(
        &self,
        record: &SearchRecord,
    ) -> Result<SearchConnection, AppError> {
        let api_key = record
            .sealed_key
            .as_deref()
            .map(|sealed| {
                self.open_secret(
                    super::SEARCH_KEY_PURPOSE,
                    record.config.kind.as_str(),
                    sealed,
                )
            })
            .transpose()?;
        Ok(SearchConnection {
            kind: record.config.kind,
            base_url: record.config.base_url.clone(),
            api_key,
        })
    }

    pub(crate) fn connector_connection(
        &self,
        connector: &(Connector, Vec<u8>),
    ) -> Result<ConnectorConnection, AppError> {
        let token = self.open_secret(
            super::CONNECTOR_SECRET_PURPOSE,
            connector.0.kind.as_str(),
            &connector.1,
        )?;
        Ok(ConnectorConnection { token })
    }

    /// Every listed model with its provider and suitability (`list_models`).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn config_list_models(&self) -> Result<Vec<ListedModel>, AppError> {
        let snapshot = self.snapshot().await?;
        Ok(snapshot
            .providers
            .iter()
            .flat_map(|record| {
                record.models.iter().map(|model| ListedModel {
                    provider_name: record.provider.name.as_str().to_owned(),
                    warnings: model.capabilities.suitability_warnings(),
                    model: model.clone(),
                })
            })
            .collect())
    }

    /// The two default model choices (`defaults`).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn config_defaults(&self) -> Result<ModelDefaults, AppError> {
        Ok(self.snapshot().await?.defaults)
    }

    /// The provider connection for a model, with the key opened for this call (`resolve_model`).
    ///
    /// # Errors
    /// [`AppError::NotFound`] for an unknown model.
    pub async fn config_resolve_model(
        &self,
        id: ProviderModelId,
    ) -> Result<ResolvedModel, AppError> {
        let snapshot = self.snapshot().await?;
        let (record, model) = snapshot
            .providers
            .iter()
            .find_map(|record| {
                record
                    .models
                    .iter()
                    .find(|model| model.id == id)
                    .map(|model| (record, model))
            })
            .ok_or(AppError::NotFound)?;
        Ok(ResolvedModel {
            provider: record.provider.clone(),
            model: model.clone(),
            connection: self.provider_connection(record)?,
            warnings: model.capabilities.suitability_warnings(),
        })
    }

    /// The active search provider's connection, if search is set up (`active_search`).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage or the vault fails.
    pub async fn config_active_search(&self) -> Result<Option<SearchConnection>, AppError> {
        let snapshot = self.snapshot().await?;
        let Some(kind) = snapshot.search_active else {
            return Ok(None);
        };
        snapshot
            .search
            .iter()
            .find(|record| record.config.kind == kind)
            .map(|record| self.search_connection(record))
            .transpose()
    }

    /// The GitHub token for clone, push and pull requests, for workers only (`github_credentials`).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage or the vault fails.
    pub async fn config_github_credentials(&self) -> Result<Option<ConnectorConnection>, AppError> {
        let snapshot = self.snapshot().await?;
        snapshot
            .connector
            .as_ref()
            .map(|connector| self.connector_connection(connector))
            .transpose()
    }

    /// The policy a run captures when it starts (`effective_policy`): the instance policy, or a
    /// session's own policy tightened to fit it (BR-18.8). The instance policy's version is kept
    /// so the run can record which one it used (D5).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn config_effective_policy(
        &self,
        session: Option<&SandboxPolicy>,
    ) -> Result<SandboxPolicy, AppError> {
        let instance = self.snapshot().await?.policy.clone();
        Ok(match session {
            Some(session) => SandboxPolicy {
                version: instance.version,
                ..session.clamped_to(&instance)
            },
            None => instance,
        })
    }
}
