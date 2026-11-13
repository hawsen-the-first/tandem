//! Default models (W19; US-2.4).

use super::providers::ModelView;
use crate::context::RequestContext;
use crate::deps::App;
use crate::error::{AppError, FieldIssue};
use crate::validation::fail_if_any;
use std::collections::HashMap;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::config::{ModelDefaults, ProviderModel, SuitabilityWarning};
use tandem_core::ids::ProviderModelId;
use tandem_core::permissions::Action;
use tandem_ports::Tx;

/// The two picks as the admin made them; either may be left unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DefaultsInput {
    /// The orchestrator's model.
    pub orchestrator: Option<ProviderModelId>,
    /// The model for thread and task agents.
    pub agents: Option<ProviderModelId>,
}

/// One default with where it comes from and what is wrong with it (BR-15.2, BR-15.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultChoice {
    /// The model.
    pub model: ProviderModel,
    /// The provider it belongs to.
    pub provider_name: String,
    /// Empty when suitable. A default with warnings is allowed.
    pub warnings: Vec<SuitabilityWarning>,
}

/// The current defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefaultsView {
    /// The orchestrator default.
    pub orchestrator: Option<DefaultChoice>,
    /// The agents default.
    pub agents: Option<DefaultChoice>,
}

impl DefaultsView {
    fn describe(choice: &Option<DefaultChoice>) -> String {
        choice.as_ref().map_or_else(
            || "not set".to_owned(),
            |choice| format!("{} / {}", choice.provider_name, choice.model.name.as_str()),
        )
    }
}

impl App {
    async fn defaults_view(
        tx: &mut dyn Tx,
        defaults: &ModelDefaults,
    ) -> Result<DefaultsView, AppError> {
        let providers: HashMap<_, _> = tx
            .providers()
            .list()
            .await?
            .into_iter()
            .map(|provider| (provider.id, provider.name.as_str().to_owned()))
            .collect();
        let mut choose =
            async |id: Option<ProviderModelId>| -> Result<Option<DefaultChoice>, AppError> {
                let Some(id) = id else { return Ok(None) };
                let Some(model) = tx.providers().get_model(id).await? else {
                    return Ok(None);
                };
                let provider_name = providers
                    .get(&model.provider_id)
                    .cloned()
                    .unwrap_or_default();
                let view = ModelView::of(model);
                Ok(Some(DefaultChoice {
                    model: view.model,
                    provider_name,
                    warnings: view.warnings,
                }))
            };
        Ok(DefaultsView {
            orchestrator: choose(defaults.orchestrator).await?,
            agents: choose(defaults.agents).await?,
        })
    }

    /// The default models with their suitability.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn get_defaults(&self, ctx: &RequestContext) -> Result<DefaultsView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let defaults = tx.defaults().get().await?;
        Self::defaults_view(tx.as_mut(), &defaults).await
    }

    /// Choose the default models. An unsuitable or untested model is allowed; its warnings are
    /// returned and recorded (BR-15.2). Sessions created afterwards use the new choice (BR-15.4).
    ///
    /// # Errors
    /// [`AppError::Validation`] when a pick is not a listed model.
    pub async fn set_defaults(
        &self,
        ctx: &RequestContext,
        input: DefaultsInput,
    ) -> Result<DefaultsView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut issues = Vec::new();
        for (field, pick) in [
            ("orchestrator", input.orchestrator),
            ("agents", input.agents),
        ] {
            if let Some(id) = pick
                && tx.providers().get_model(id).await?.is_none()
            {
                issues.push(FieldIssue {
                    field,
                    message: "Choose one of the listed models".to_owned(),
                });
            }
        }
        fail_if_any(issues)?;
        let before_defaults = tx.defaults().get().await?;
        let before = Self::defaults_view(tx.as_mut(), &before_defaults).await?;
        let new_defaults = ModelDefaults {
            orchestrator: input.orchestrator,
            agents: input.agents,
        };
        tx.defaults()
            .save(&new_defaults, actor.user_id, now)
            .await?;
        let after = Self::defaults_view(tx.as_mut(), &new_defaults).await?;
        let warning_count = |choice: &Option<DefaultChoice>| {
            choice
                .as_ref()
                .map_or(0, |choice| choice.warnings.len())
                .to_string()
        };
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigDefaultsUpdated,
                AuditOutcome::Success,
            )
            .object("model_defaults", "instance")
            .detail(
                "orchestrator_before",
                DefaultsView::describe(&before.orchestrator),
            )
            .detail(
                "orchestrator_after",
                DefaultsView::describe(&after.orchestrator),
            )
            .detail("agents_before", DefaultsView::describe(&before.agents))
            .detail("agents_after", DefaultsView::describe(&after.agents))
            .detail("orchestrator_warnings", warning_count(&after.orchestrator))
            .detail("agents_warnings", warning_count(&after.agents));
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(after)
    }
}
