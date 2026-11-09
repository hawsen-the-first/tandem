//! The outbox relay's single-leader lock (NFR design 6). A PostgreSQL session-level advisory lock
//! held on a dedicated connection means exactly one relay is active at a time, so event order is
//! preserved even with several workers.

use crate::error::map_sqlx;
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use tandem_ports::StoreError;

const RELAY_LOCK_KEY: i64 = 0x7A6E_6465_6D03;

/// Proof that this process is the active relay. Release it explicitly; if it is dropped without
/// release, its connection is closed, which also releases the lock.
#[derive(Debug)]
pub struct RelayLease {
    connection: Option<PoolConnection<Postgres>>,
}

impl RelayLease {
    /// Try to become the active relay. Returns `None` when another process holds the lock.
    ///
    /// # Errors
    /// [`StoreError`] when no connection is available.
    pub async fn try_acquire(pool: &PgPool) -> Result<Option<Self>, StoreError> {
        let mut connection = pool.acquire().await.map_err(map_sqlx)?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(RELAY_LOCK_KEY)
            .fetch_one(&mut *connection)
            .await
            .map_err(map_sqlx)?;
        if acquired {
            Ok(Some(Self {
                connection: Some(connection),
            }))
        } else {
            Ok(None)
        }
    }

    /// Give up leadership.
    ///
    /// # Errors
    /// [`StoreError`] when the unlock statement fails (the connection is then closed anyway).
    pub async fn release(mut self) -> Result<(), StoreError> {
        if let Some(mut connection) = self.connection.take() {
            let result = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(RELAY_LOCK_KEY)
                .execute(&mut *connection)
                .await
                .map_err(map_sqlx);
            if result.is_err() {
                connection.close_on_drop();
            }
            result?;
        }
        Ok(())
    }
}

impl Drop for RelayLease {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.as_mut() {
            connection.close_on_drop();
        }
    }
}
