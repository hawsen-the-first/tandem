//! Reversible migrations (AV-4, NFR-5.5) and the start-up schema guard (AV-3).

use sqlx::PgPool;
use sqlx::migrate::Migrator;

/// The migrations embedded in this binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The newest migration version this binary expects.
pub fn expected_schema_version() -> i64 {
    MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .map(|m| m.version)
        .max()
        .unwrap_or(0)
}

/// Why the schema check failed.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The database is not at the version this binary expects.
    #[error(
        "the database schema is at version {actual} but this build expects {expected}; run `tandem migrate`"
    )]
    Mismatch {
        /// What the binary expects.
        expected: i64,
        /// What the database has.
        actual: i64,
    },
    /// The version could not be read.
    #[error("could not read the schema version: {0}")]
    Read(#[source] sqlx::Error),
}

/// The newest successfully applied version (0 when none).
///
/// # Errors
/// Any SQLx error, including a missing migrations table.
pub async fn applied_schema_version(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let version = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(version) FROM _sqlx_migrations WHERE success",
    )
    .fetch_one(pool)
    .await?;
    Ok(version.unwrap_or(0))
}

/// Refuse to serve when the schema is older or newer than this binary expects (AV-3).
///
/// # Errors
/// [`SchemaError`].
pub async fn check_schema(pool: &PgPool) -> Result<(), SchemaError> {
    let actual = applied_schema_version(pool)
        .await
        .map_err(SchemaError::Read)?;
    let expected = expected_schema_version();
    if actual == expected {
        Ok(())
    } else {
        Err(SchemaError::Mismatch { expected, actual })
    }
}

/// Apply every pending migration. Run under the migration role only.
///
/// # Errors
/// Any migration error.
pub async fn migrate_up(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

/// Revert the most recent migration. Returns the version that was reverted, if any.
///
/// # Errors
/// Any migration error.
pub async fn migrate_down_one(pool: &PgPool) -> Result<Option<i64>, sqlx::migrate::MigrateError> {
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(pool)
            .await
            .map_err(sqlx::migrate::MigrateError::Execute)?;
    let Some(latest) = applied.last().copied() else {
        return Ok(None);
    };
    let target = applied.iter().rev().nth(1).copied().unwrap_or(0);
    MIGRATOR.undo(pool, target).await?;
    Ok(Some(latest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn av_4_every_migration_has_an_up_and_a_down_script() {
        let ups: Vec<i64> = MIGRATOR
            .iter()
            .filter(|m| m.migration_type.is_up_migration())
            .map(|m| m.version)
            .collect();
        let downs: Vec<i64> = MIGRATOR
            .iter()
            .filter(|m| m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect();
        assert_eq!(ups, downs);
        assert_eq!(expected_schema_version(), *ups.iter().max().unwrap());
    }
}
