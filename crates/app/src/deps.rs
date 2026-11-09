//! The application object: all services hang off [`App`].

use std::sync::Arc;
use tandem_ports::{BreachChecker, ModelProvider, PasswordHasher, Store, TotpService, Vault};
use tandem_support::{Clock, Rng};

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
