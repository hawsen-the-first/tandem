//! Shared process bootstrap for the api, worker and cli: pools, vault, start-up guards, readiness
//! probes, shutdown and supervised loops.

mod bootstrap;
mod probes;
mod shutdown;

pub use bootstrap::{
    HASH_PROFILE_KEY, Process, Services, StartupError, VAULT_CANARY_KEY, format_hash_profile,
    init_observability, load_vault, parse_hash_profile,
};
pub use probes::{DatabaseProbe, SchemaProbe, VaultProbe, spawn_pool_metrics};
pub use shutdown::{Shutdown, wait_for_termination};

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-runtime";

#[cfg(test)]
mod tests;
