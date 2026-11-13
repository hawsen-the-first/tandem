//! The application object: all services hang off [`App`].

use crate::integrations::ConfigCache;
use std::sync::Arc;
use tandem_ports::{
    BreachChecker, ConnectorProbe, Locks, ModelProvider, PasswordHasher, ProviderProbe,
    RuntimeProbe, SearchProbe, Store, TotpService, Vault,
};
use tandem_support::{Clock, Rng};

/// What the configuration tests need from the outside world: a lock per target and one probe for
/// each kind of thing an admin can test (U2).
#[derive(Clone)]
pub struct Integrations {
    /// Session locks, one per test target.
    pub locks: Arc<dyn Locks>,
    /// Tests a model provider and its models.
    pub provider_probe: Arc<dyn ProviderProbe>,
    /// Tests the search provider.
    pub search_probe: Arc<dyn SearchProbe>,
    /// Tests the GitHub connector.
    pub connector_probe: Arc<dyn ConnectorProbe>,
    /// Checks the container runtime.
    pub runtime_probe: Arc<dyn RuntimeProbe>,
}

/// Everything the application needs from the outside world.
pub struct Dependencies {
    /// Opens transactions.
    pub store: Arc<dyn Store>,
    /// Time source.
    pub clock: Arc<dyn Clock>,
    /// Random source.
    pub rng: Arc<dyn Rng>,
    /// Password hashing.
    pub hasher: Arc<dyn PasswordHasher>,
    /// Breached-password check.
    pub breach: Arc<dyn BreachChecker>,
    /// TOTP.
    pub totp: Arc<dyn TotpService>,
    /// Secret vault.
    pub vault: Arc<dyn Vault>,
    /// Model provider used by the skeleton's agent.
    pub provider: Arc<dyn ModelProvider>,
    /// Issuer name shown in authenticator apps.
    pub issuer: String,
    /// The configuration test machinery.
    pub integrations: Integrations,
}

/// The application services.
#[derive(Clone)]
pub struct App {
    pub(crate) store: Arc<dyn Store>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) rng: Arc<dyn Rng>,
    pub(crate) hasher: Arc<dyn PasswordHasher>,
    pub(crate) breach: Arc<dyn BreachChecker>,
    pub(crate) totp: Arc<dyn TotpService>,
    pub(crate) vault: Arc<dyn Vault>,
    pub(crate) provider: Arc<dyn ModelProvider>,
    pub(crate) issuer: String,
    pub(crate) integrations: Integrations,
    pub(crate) config_cache: ConfigCache,
}

impl App {
    /// Build the application from its dependencies.
    pub fn new(dependencies: Dependencies) -> Self {
        Self {
            store: dependencies.store,
            clock: dependencies.clock,
            rng: dependencies.rng,
            hasher: dependencies.hasher,
            breach: dependencies.breach,
            totp: dependencies.totp,
            vault: dependencies.vault,
            provider: dependencies.provider,
            issuer: dependencies.issuer,
            integrations: dependencies.integrations,
            config_cache: ConfigCache::default(),
        }
    }
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Dependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dependencies").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Integrations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Integrations").finish_non_exhaustive()
    }
}
