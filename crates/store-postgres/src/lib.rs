//! PostgreSQL adapter: secure pool factory, reversible migrations, schema guard, repositories, job
//! queue, outbox and cleanup. All SQL is parameterised (SECURITY-05).

mod audit;
mod chat;
mod cleanup;
mod config_store;
mod convert;
mod error;
mod guards;
mod identity;
mod internal;
mod jobs;
mod migrate;
mod outbox;
mod pool;
mod providers;
mod relay;
mod settings;
mod stats;
mod store;
mod test_runs;

pub use cleanup::CleanupReport;
pub use error::map_sqlx;
pub use internal::{RekeyError, RekeyReport};
pub use migrate::{
    MIGRATOR, SchemaError, applied_schema_version, check_schema, expected_schema_version,
    migrate_down_one, migrate_up,
};
pub use pool::{PoolConfig, PoolError, connect_pool};
pub use relay::RelayLease;
pub use stats::{JobStats, OutboxStats};
pub use store::{PgStore, PgTx};
pub use test_runs::PgLocks;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-store-postgres";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
