//! Search settings and the GitHub connector (W20, W21; US-2.5, US-2.6).

use super::{CONNECTOR_SECRET_PURPOSE, SEARCH_KEY_PURPOSE, SecretInput, check_secret, set_text};
use crate::context::RequestContext;
use crate::deps::App;
use crate::error::{AppError, FieldIssue};
use crate::validation::fail_if_any;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::config::{
    Connector, ConnectorAuth, ConnectorKind, DEFAULT_SEARXNG_URL, SearchConfig, SearchKind,
    TestRun, TestStatus, TestTarget,
};
use tandem_core::ids::TestRunId;
use tandem_core::outbound_url::OutboundUrl;
use tandem_core::permissions::Action;
use tandem_ports::{SecretChange, Tx};
use tandem_support::Secret;
use time::OffsetDateTime;

/// Search settings as the admin entered them.
#[derive(Debug, Clone)]
pub struct SearchInput {
    /// Which provider.
    pub kind: SearchKind,
    /// SearXNG's base URL; empty or absent means the bundled service's address.
    pub base_url: Option<String>,
    /// What to do with the API key (Brave and Tavily).
    pub key: SecretInput,
}

/// One search kind's settings and test state. The key is never part of it (BR-20.2).
#[derive(Debug, Clone)]
pub struct SearchView {
    /// Which provider.
    pub kind: SearchKind,
    /// Settings were saved for this kind.
    pub configured: bool,
    /// SearXNG's base URL.
    pub base_url: Option<OutboundUrl>,
    /// When a key was saved.
    pub key_saved_at: Option<OffsetDateTime>,
    /// The newest finished test.
    pub last_test: Option<TestRun>,
    /// A test that is queued or running now.
    pub active_test: Option<TestRunId>,
}

/// All three kinds and which one is active (BR-16.1).
#[derive(Debug, Clone)]
pub struct SearchOverview {
    /// One entry per kind, in display order.
    pub kinds: Vec<SearchView>,
    /// The active kind, if any.
    pub active: Option<SearchKind>,
}

/// The outcome of activating or deactivating search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchActivation {
    /// The active kind afterwards.
    pub active: Option<SearchKind>,
    /// The provider has never passed a test (BR-16.4): the screen shows a warning.
    pub untested: bool,
}

/// The connector as the admin may see it (BR-20.2).
#[derive(Debug, Clone)]
pub struct ConnectorView {
    /// The connector (carries `secret_saved_at`, the account and the expiry).
    pub connector: Connector,
    /// The newest finished test.
    pub last_test: Option<TestRun>,
    /// A test that is queued or running now.
    pub active_test: Option<TestRunId>,
}

fn passed(run: &Option<TestRun>) -> bool {
    run.as_ref().is_some_and(|run| {
        !run.results.is_empty()
            && run
                .results
                .iter()
                .all(|result| result.status == TestStatus::Ok)
    })
}

impl App {
    async fn search_view(
        tx: &mut dyn Tx,
        kind: SearchKind,
        config: Option<SearchConfig>,
    ) -> Result<SearchView, AppError> {
        let target = TestTarget::Search(kind);
        let last_test = tx.test_runs().latest_finished(target).await?;
        let active_test = tx.test_runs().active_for(target).await?.map(|run| run.id);
        Ok(SearchView {
            kind,
            configured: config.is_some(),
            base_url: config.as_ref().and_then(|config| config.base_url.clone()),
            key_saved_at: config.and_then(|config| config.key_saved_at),
            last_test,
            active_test,
        })
    }

    /// All search kinds with their settings, test state and which is active.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn get_search(&self, ctx: &RequestContext) -> Result<SearchOverview, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let configs = tx.search().list().await?;
        let mut kinds = Vec::new();
        for kind in SearchKind::ALL {
            let config = configs.iter().find(|config| config.kind == kind).cloned();
            kinds.push(Self::search_view(tx.as_mut(), kind, config).await?);
        }
        let active = tx.search().active().await?;
        Ok(SearchOverview { kinds, active })
    }

    /// Save one kind's settings. Settings of inactive kinds are kept (BR-16.3).
    ///
    /// # Errors
    /// [`AppError::Validation`] for a bad URL or key.
    pub async fn save_search(
        &self,
        ctx: &RequestContext,
        input: SearchInput,
    ) -> Result<SearchView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let mut issues = Vec::new();
        check_secret("api_key", &input.key, &mut issues);
        let base_url = if input.kind.needs_url() {
            let text = input
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .unwrap_or(DEFAULT_SEARXNG_URL);
            match OutboundUrl::parse(text) {
                Ok(url) => Some(url),
                Err(error) => {
                    issues.push(FieldIssue {
                        field: "base_url",
                        message: error.to_string(),
                    });
                    None
                }
            }
        } else {
            None
        };
        if !input.kind.needs_key() && !matches!(input.key, SecretInput::Keep) {
            issues.push(FieldIssue {
                field: "api_key",
                message: "This search provider does not use a key".to_owned(),
            });
        }
        fail_if_any(issues)?;

        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let previous = tx.search().get(input.kind).await?;
        let (change, key_saved_at) = match &input.key {
            SecretInput::Keep => (
                SecretChange::Keep,
                previous.as_ref().and_then(|config| config.key_saved_at),
            ),
            SecretInput::Set(secret) => (
                SecretChange::Set(self.vault.seal(
                    SEARCH_KEY_PURPOSE,
                    input.kind.as_str(),
                    secret.expose().as_bytes(),
                )?),
                Some(now),
            ),
            SecretInput::Remove => (SecretChange::Clear, None),
        };
        let config = SearchConfig {
            kind: input.kind,
            base_url,
            key_saved_at,
            updated_at: now,
            updated_by: actor.user_id,
        };
        tx.search().save(&config, change).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigSearchUpdated,
                AuditOutcome::Success,
            )
            .object("search", input.kind.as_str())
            .detail("action", "saved")
            .detail("kind", input.kind.as_str())
            .detail(
                "base_url",
                config.base_url.as_ref().map_or("none", OutboundUrl::as_str),
            )
            .detail(
                "api_key_before",
                set_text(
                    previous
                        .as_ref()
                        .is_some_and(|config| config.key_saved_at.is_some()),
                ),
            )
            .detail("api_key_after", set_text(config.key_saved_at.is_some()));
        self.append_audit(tx.as_mut(), event).await?;
        let view = Self::search_view(tx.as_mut(), input.kind, Some(config)).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(view)
    }

    /// Make one kind the single active provider, or turn search off with `None` (W20).
    ///
    /// # Errors
    /// [`AppError::Validation`] when the kind needs a key that is not saved.
    pub async fn set_active_search(
        &self,
        ctx: &RequestContext,
        kind: Option<SearchKind>,
    ) -> Result<SearchActivation, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let before = tx.search().active().await?;
        let mut untested = false;
        if let Some(kind) = kind {
            let existing = tx.search().get(kind).await?;
            match existing {
                Some(config) if kind.needs_key() && config.key_saved_at.is_none() => {
                    return Err(AppError::validation(
                        "kind",
                        "Save an API key before turning this on",
                    ));
                }
                Some(_) => {}
                None if kind.needs_key() => {
                    return Err(AppError::validation(
                        "kind",
                        "Save an API key before turning this on",
                    ));
                }
                None => {
                    let default_url = OutboundUrl::parse(DEFAULT_SEARXNG_URL)
                        .map_err(|error| AppError::Internal(error.to_string()))?;
                    let config = SearchConfig {
                        kind,
                        base_url: Some(default_url),
                        key_saved_at: None,
                        updated_at: now,
                        updated_by: actor.user_id,
                    };
                    tx.search().save(&config, SecretChange::Keep).await?;
                }
            }
            let last = tx
                .test_runs()
                .latest_finished(TestTarget::Search(kind))
                .await?;
            untested = !passed(&last);
        }
        tx.search().set_active(kind, actor.user_id, now).await?;
        let label = |kind: Option<SearchKind>| kind.map_or("none", SearchKind::as_str);
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigSearchUpdated,
                AuditOutcome::Success,
            )
            .object("search", label(kind))
            .detail(
                "action",
                if kind.is_some() {
                    "activated"
                } else {
                    "deactivated"
                },
            )
            .detail("active_before", label(before))
            .detail("active_after", label(kind))
            .detail("untested", untested.to_string());
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(SearchActivation {
            active: kind,
            untested,
        })
    }

    /// The GitHub connector, if configured.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn get_connector(
        &self,
        ctx: &RequestContext,
    ) -> Result<Option<ConnectorView>, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        let Some(connector) = tx.connectors().get(ConnectorKind::Github).await? else {
            return Ok(None);
        };
        Ok(Some(Self::connector_view(tx.as_mut(), connector).await?))
    }

    async fn connector_view(
        tx: &mut dyn Tx,
        connector: Connector,
    ) -> Result<ConnectorView, AppError> {
        let target = TestTarget::Connector(connector.kind);
        let last_test = tx.test_runs().latest_finished(target).await?;
        let active_test = tx.test_runs().active_for(target).await?.map(|run| run.id);
        Ok(ConnectorView {
            connector,
            last_test,
            active_test,
        })
    }

    /// Save (or replace) the GitHub token (W21). A replaced token forgets the old account and
    /// expiry until the next test.
    ///
    /// # Errors
    /// [`AppError::Validation`] for an empty or malformed token.
    pub async fn save_connector_token(
        &self,
        ctx: &RequestContext,
        token: Secret,
    ) -> Result<ConnectorView, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let input = SecretInput::Set(Secret::new(token.expose().trim()));
        let mut issues = Vec::new();
        if token.expose().trim().is_empty() {
            issues.push(FieldIssue {
                field: "token",
                message: "Paste the access token".to_owned(),
            });
        }
        check_secret("token", &input, &mut issues);
        fail_if_any(issues)?;
        let sealed = self.vault.seal(
            CONNECTOR_SECRET_PURPOSE,
            ConnectorKind::Github.as_str(),
            token.expose().trim().as_bytes(),
        )?;
        let now = self.clock.now();
        let connector = Connector {
            kind: ConnectorKind::Github,
            auth: ConnectorAuth::Token,
            secret_saved_at: now,
            account_login: None,
            token_expires_at: None,
            updated_at: now,
            updated_by: actor.user_id,
        };
        let mut tx = self.store.begin().await?;
        let replaced = tx.connectors().get(ConnectorKind::Github).await?.is_some();
        tx.connectors().save(&connector, &sealed).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigConnectorUpdated,
                AuditOutcome::Success,
            )
            .object("connector", ConnectorKind::Github.as_str())
            .detail("action", if replaced { "replaced" } else { "saved" })
            .detail("kind", ConnectorKind::Github.as_str())
            .detail("credential_before", set_text(replaced))
            .detail("credential_after", "set");
        self.append_audit(tx.as_mut(), event).await?;
        let view = Self::connector_view(tx.as_mut(), connector).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(view)
    }

    /// Remove the GitHub token; the connector is then off (BR-17.6).
    ///
    /// # Errors
    /// [`AppError::NotFound`] when no connector is configured.
    pub async fn remove_connector(&self, ctx: &RequestContext) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        tx.connectors()
            .delete(ConnectorKind::Github)
            .await
            .map_err(|error| match error {
                tandem_ports::StoreError::NotFound => AppError::NotFound,
                other => other.into(),
            })?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigConnectorUpdated,
                AuditOutcome::Success,
            )
            .object("connector", ConnectorKind::Github.as_str())
            .detail("action", "removed")
            .detail("kind", ConnectorKind::Github.as_str())
            .detail("credential_before", "set")
            .detail("credential_after", "not set");
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(())
    }
}
