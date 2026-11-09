//! PostgreSQL adapter: secure pool factory, reversible migrations, schema guard, repositories, job
//! queue, outbox and cleanup. All SQL is parameterised (SECURITY-05).

mod audit;
mod chat;
mod cleanup;
mod convert;
mod error;
mod guards;
mod identity;
mod jobs;
mod migrate;
mod outbox;
mod pool;
mod relay;
mod settings;
mod store;

pub use cleanup::CleanupReport;
pub use error::map_sqlx;
pub use migrate::{
    MIGRATOR, SchemaError, applied_schema_version, check_schema, expected_schema_version,
    migrate_down_one, migrate_up,
};
pub use pool::{PoolConfig, PoolError, connect_pool};
pub use relay::RelayLease;
pub use store::{PgStore, PgTx};

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
