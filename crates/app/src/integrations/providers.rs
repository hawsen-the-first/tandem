//! Providers and their models (W16, W18; US-2.1).

use super::{PROVIDER_KEY_PURPOSE, SecretInput, check_secret, roles_text, set_text};
use crate::context::RequestContext;
use crate::deps::App;
use crate::error::{AppError, FieldIssue};
use crate::validation::fail_if_any;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::config::{
    DefaultRole, MAX_PROVIDERS, ModelCapabilities, ModelName, Provider, ProviderKind,
    ProviderModel, ProviderName, SuitabilityWarning, TestRun, TestTarget, ToolCalling,
    validate_model_names,
};
use tandem_core::ids::{ProviderId, ProviderModelId, TestRunId};
use tandem_core::outbound_url::OutboundUrl;
use tandem_core::permissions::Action;
use tandem_ports::{SecretChange, StoreError, Tx};

/// One model as the admin entered it.
#[derive(Debug, Clone)]
pub struct ModelInput {
    /// The existing model this row edits; none for a new one.
    pub id: Option<ProviderModelId>,
    /// The name the provider's API expects.
    pub name: String,
    /// An optional friendly label; empty means none.
    pub label: String,
    /// What the admin declares about tool calling, if anything.
    pub declared_tool_calling: Option<ToolCalling>,
    /// What the admin declares about the context length, if anything.
    pub declared_context_tokens: Option<u32>,
}

/// A provider as the admin entered it.
#[derive(Debug, Clone)]
pub struct ProviderInput {
    /// Display name.
    pub name: String,
    /// Base URL.
    pub base_url: String,
    /// What to do with the API key.
    pub key: SecretInput,
    /// The listed models, in order.
    pub models: Vec<ModelInput>,
}

/// A model with the reasons it may not suit a job (BR-14.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelView {
    /// The model.
    pub model: ProviderModel,
    /// Empty when suitable.
    pub warnings: Vec<SuitabilityWarning>,
    /// A declared value disagrees with what a test found.
    pub disagrees_with_detection: bool,
}

impl ModelView {
    pub(crate) fn of(model: ProviderModel) -> Self {
        Self {
            warnings: model.capabilities.suitability_warnings(),
            disagrees_with_detection: model.capabilities.disagrees_with_detection(),
            model,
        }
    }
}

/// A provider with its models and test state. The key itself is never part of it (BR-20.2).
#[derive(Debug, Clone)]
pub struct ProviderView {
    /// The provider (carries `key_saved_at`).
    pub provider: Provider,
    /// Its models.
    pub models: Vec<ModelView>,
    /// The newest finished test.
    pub last_test: Option<TestRun>,
    /// A test that is queued or running now.
    pub active_test: Option<TestRunId>,
}

/// What removing a provider would do (W18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalImpact {
    /// The provider's name.
    pub provider_name: String,
    /// How many models go with it.
    pub model_count: usize,
    /// Defaults that will be cleared.
    pub cleared_defaults: Vec<DefaultRole>,
}

struct Parsed {
    name: ProviderName,
    base_url: OutboundUrl,
    models: Vec<ParsedModel>,
}

struct ParsedModel {
    id: Option<ProviderModelId>,
    name: ModelName,
    label: Option<String>,
    declared_tool_calling: Option<ToolCalling>,
    declared_context_tokens: Option<u32>,
}

fn parse_input(input: &ProviderInput) -> Result<Parsed, AppError> {
    let mut issues = Vec::new();
    let name = ProviderName::parse(&input.name)
        .map_err(|error| {
            issues.push(FieldIssue {
                field: "name",
                message: error.to_string(),
            })
        })
        .ok();
    let base_url = OutboundUrl::parse(&input.base_url)
        .map_err(|error| {
            issues.push(FieldIssue {
                field: "base_url",
                message: error.to_string(),
            })
        })
        .ok();
    check_secret("api_key", &input.key, &mut issues);
    let mut models = Vec::new();
    for entry in &input.models {
        let model_name = match ModelName::parse(&entry.name) {
            Ok(name) => Some(name),
            Err(error) => {
                issues.push(FieldIssue {
                    field: "models",
                    message: error.to_string(),
                });
                None
            }
        };
        let label = match ProviderModel::clean_label(&entry.label) {
            Ok(label) => label,
            Err(error) => {
                issues.push(FieldIssue {
                    field: "models",
                    message: error.to_string(),
                });
                None
            }
        };
        if let Some(tokens) = entry.declared_context_tokens {
            let mut probe = ModelCapabilities::default();
            if let Err(error) = probe.declare_context_tokens(tokens) {
                issues.push(FieldIssue {
                    field: "models",
                    message: error.to_string(),
                });
            }
        }
        if let Some(name) = model_name {
            models.push(ParsedModel {
                id: entry.id,
                name,
                label,
                declared_tool_calling: entry.declared_tool_calling,
                declared_context_tokens: entry.declared_context_tokens,
            });
        }
    }
    let names: Vec<ModelName> = models.iter().map(|model| model.name.clone()).collect();
    if let Err(error) = validate_model_names(&names) {
        issues.push(FieldIssue {
            field: "models",
            message: error.to_string(),
        });
    }
    fail_if_any(issues)?;
    match (name, base_url) {
        (Some(name), Some(base_url)) => Ok(Parsed {
            name,
            base_url,
            models,
        }),
        _ => Err(AppError::Internal(
            "provider input passed validation incompletely".into(),
        )),
    }
}

fn conflict_to_validation(error: StoreError) -> AppError {
    match error {
        StoreError::Conflict("providers.name") => {
            AppError::validation("name", "A provider with this name already exists")
        }
        StoreError::Conflict("provider_models.name") => {
            AppError::validation("models", "That model is already listed")
        }
        other => other.into(),
    }
}

impl App {
    /// Every provider with its models and test state.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn list_providers(
        &self,
        ctx: &RequestContext,
    ) -> Result<Vec<ProviderView>, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let providers = tx.providers().list().await?;
        let mut views = Vec::with_capacity(providers.len());
        for provider in providers {
            views.push(Self::provider_view(tx.as_mut(), provider).await?);
        }
        Ok(views)
    }

    /// One provider.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins or an unknown provider.
    pub async fn get_provider(
        &self,
        ctx: &RequestContext,
        id: ProviderId,
    ) -> Result<ProviderView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let provider = tx.providers().get(id).await?.ok_or(AppError::NotFound)?;
        Self::provider_view(tx.as_mut(), provider).await
    }

    pub(crate) async fn provider_view(
        tx: &mut dyn Tx,
        provider: Provider,
    ) -> Result<ProviderView, AppError> {
        let models = tx.providers().models(provider.id).await?;
        let target = TestTarget::Provider(provider.id);
        let last_test = tx.test_runs().latest_finished(target).await?;
        let active_test = tx.test_runs().active_for(target).await?.map(|run| run.id);
        Ok(ProviderView {
            provider,
            models: models.into_iter().map(ModelView::of).collect(),
            last_test,
            active_test,
        })
    }

    /// Add a provider (W16).
    ///
    /// # Errors
    /// [`AppError::Validation`] with the field problems; the name must be unused.
    pub async fn create_provider(
        &self,
        ctx: &RequestContext,
        input: ProviderInput,
    ) -> Result<ProviderView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let parsed = parse_input(&input)?;
        let now = self.clock.now();
        let id = ProviderId::generate(&*self.clock, &*self.rng)?;
        let sealed = match &input.key {
            SecretInput::Set(secret) => Some(self.vault.seal(
                PROVIDER_KEY_PURPOSE,
                &id.to_string(),
                secret.expose().as_bytes(),
            )?),
            SecretInput::Keep | SecretInput::Remove => None,
        };
        let provider = Provider {
            id,
            kind: ProviderKind::OpenAiCompatible,
            name: parsed.name,
            base_url: parsed.base_url,
            key_saved_at: sealed.as_ref().map(|_| now),
            created_at: now,
            created_by: actor.user_id,
            updated_at: now,
            updated_by: actor.user_id,
        };
        let mut tx = self.store.begin().await?;
        if tx.providers().list().await?.len() >= MAX_PROVIDERS {
            return Err(AppError::validation(
                "name",
                format!("An instance can have at most {MAX_PROVIDERS} providers"),
            ));
        }
        tx.providers()
            .insert(&provider, sealed.as_deref())
            .await
            .map_err(conflict_to_validation)?;
        let mut models = Vec::with_capacity(parsed.models.len());
        for entry in parsed.models {
            models.push(ProviderModel {
                id: ProviderModelId::generate(&*self.clock, &*self.rng)?,
                provider_id: id,
                name: entry.name,
                label: entry.label,
                capabilities: ModelCapabilities {
                    declared_tool_calling: entry.declared_tool_calling,
                    declared_context_tokens: entry.declared_context_tokens,
                    ..ModelCapabilities::default()
                },
            });
        }
        tx.providers()
            .replace_models(id, &models)
            .await
            .map_err(conflict_to_validation)?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigProviderCreated,
                AuditOutcome::Success,
            )
            .object("provider", id)
            .detail("name", provider.name.as_str())
            .detail("base_url", provider.base_url.as_str())
            .detail("api_key", set_text(provider.key_saved_at.is_some()))
            .detail("models", models.len().to_string());
        self.append_audit(tx.as_mut(), event).await?;
        let view = Self::provider_view(tx.as_mut(), provider).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(view)
    }

    /// Change a provider (W16). Models no longer listed are removed and any default pointing at one
    /// is cleared (BR-15.3).
    ///
    /// # Errors
    /// [`AppError::NotFound`], [`AppError::Validation`].
    pub async fn update_provider(
        &self,
        ctx: &RequestContext,
        id: ProviderId,
        input: ProviderInput,
    ) -> Result<ProviderView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let parsed = parse_input(&input)?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let before = tx.providers().get(id).await?.ok_or(AppError::NotFound)?;
        let existing = tx.providers().models(id).await?;

        let (key_change, key_saved_at) = match &input.key {
            SecretInput::Keep => (SecretChange::Keep, before.key_saved_at),
            SecretInput::Set(secret) => (
                SecretChange::Set(self.vault.seal(
                    PROVIDER_KEY_PURPOSE,
                    &id.to_string(),
                    secret.expose().as_bytes(),
                )?),
                Some(now),
            ),
            SecretInput::Remove => (SecretChange::Clear, None),
        };
        let provider = Provider {
            name: parsed.name,
            base_url: parsed.base_url,
            key_saved_at,
            updated_at: now,
            updated_by: actor.user_id,
            ..before.clone()
        };
        tx.providers()
            .update(&provider, key_change)
            .await
            .map_err(conflict_to_validation)?;

        let mut models = Vec::with_capacity(parsed.models.len());
        for entry in parsed.models {
            let kept = entry
                .id
                .and_then(|wanted| existing.iter().find(|model| model.id == wanted));
            let (model_id, detected) = match kept {
                Some(model) => (model.id, model.capabilities),
                None => (
                    ProviderModelId::generate(&*self.clock, &*self.rng)?,
                    ModelCapabilities::default(),
                ),
            };
            models.push(ProviderModel {
                id: model_id,
                provider_id: id,
                name: entry.name,
                label: entry.label,
                capabilities: ModelCapabilities {
                    declared_tool_calling: entry.declared_tool_calling,
                    declared_context_tokens: entry.declared_context_tokens,
                    ..detected
                },
            });
        }
        let removed = tx
            .providers()
            .replace_models(id, &models)
            .await
            .map_err(conflict_to_validation)?;
        let cleared = Self::clear_defaults(tx.as_mut(), &removed, actor.user_id, now).await?;

        let mut event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigProviderUpdated,
                AuditOutcome::Success,
            )
            .object("provider", id)
            .detail("name_before", before.name.as_str())
            .detail("name_after", provider.name.as_str())
            .detail("base_url_before", before.base_url.as_str())
            .detail("base_url_after", provider.base_url.as_str())
            .detail("api_key_before", set_text(before.key_saved_at.is_some()))
            .detail("api_key_after", set_text(provider.key_saved_at.is_some()))
            .detail("models_before", existing.len().to_string())
            .detail("models_after", models.len().to_string())
            .detail("models_removed", removed.len().to_string());
        if !cleared.is_empty() {
            event = event.detail("cleared_defaults", roles_text(&cleared));
        }
        self.append_audit(tx.as_mut(), event).await?;
        let view = Self::provider_view(tx.as_mut(), provider).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(view)
    }

    /// Clear every default that points at one of `removed`; returns the roles cleared.
    pub(crate) async fn clear_defaults(
        tx: &mut dyn Tx,
        removed: &[ProviderModelId],
        by: tandem_core::ids::UserId,
        now: time::OffsetDateTime,
    ) -> Result<Vec<DefaultRole>, AppError> {
        if removed.is_empty() {
            return Ok(Vec::new());
        }
        let mut defaults = tx.defaults().get().await?;
        let cleared = defaults.clear_models(removed);
        if !cleared.is_empty() {
            tx.defaults().save(&defaults, by, now).await?;
        }
        Ok(cleared)
    }

    /// What removing a provider would do, for the confirmation (W18).
    ///
    /// # Errors
    /// [`AppError::NotFound`].
    pub async fn provider_removal_impact(
        &self,
        ctx: &RequestContext,
        id: ProviderId,
    ) -> Result<RemovalImpact, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let provider = tx.providers().get(id).await?.ok_or(AppError::NotFound)?;
        let models = tx.providers().models(id).await?;
        let ids: Vec<ProviderModelId> = models.iter().map(|model| model.id).collect();
        let mut defaults = tx.defaults().get().await?;
        let cleared_defaults = defaults.clear_models(&ids);
        Ok(RemovalImpact {
            provider_name: provider.name.as_str().to_owned(),
            model_count: models.len(),
            cleared_defaults,
        })
    }

    /// Remove a provider, its models and key, and clear the defaults that used them (W18).
    ///
    /// # Errors
    /// [`AppError::NotFound`].
    pub async fn delete_provider(
        &self,
        ctx: &RequestContext,
        id: ProviderId,
    ) -> Result<RemovalImpact, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let provider = tx.providers().get(id).await?.ok_or(AppError::NotFound)?;
        let models = tx.providers().models(id).await?;
        let ids: Vec<ProviderModelId> = models.iter().map(|model| model.id).collect();
        let cleared = Self::clear_defaults(tx.as_mut(), &ids, actor.user_id, now).await?;
        tx.providers().delete(id).await?;
        let mut event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigProviderDeleted,
                AuditOutcome::Success,
            )
            .object("provider", id)
            .detail("name", provider.name.as_str())
            .detail("api_key", set_text(provider.key_saved_at.is_some()))
            .detail("models_removed", models.len().to_string());
        if !cleared.is_empty() {
            event = event.detail("cleared_defaults", roles_text(&cleared));
        }
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(RemovalImpact {
            provider_name: provider.name.as_str().to_owned(),
            model_count: models.len(),
            cleared_defaults: cleared,
        })
    }
}
