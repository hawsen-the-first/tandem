//! The PostgreSQL transaction (the `Store` and `Tx` ports).

use crate::error::map_sqlx;
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use tandem_ports::{
    AuditRepo, ChatRepo, ConnectorRepo, DefaultsRepo, GuardRepo, InvitationRepo, JobQueue, MfaRepo,
    OutboxRepo, PasswordResetRepo, PolicyRepo, ProviderRepo, SearchRepo, SessionRepo, SettingsRepo,
    Store, StoreError, TestRunRepo, Tx, UserRepo,
};

/// Advisory-lock key serialising first-run setup.
const SETUP_LOCK_KEY: i64 = 0x7A6E_6465_6D01;
/// Advisory-lock key serialising admin-count-sensitive changes.
const ADMIN_LOCK_KEY: i64 = 0x7A6E_6465_6D02;

/// Opens PostgreSQL transactions on a pool.
#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    /// Wrap a pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The underlying pool (for health checks and maintenance).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl Store for PgStore {
    async fn begin(&self) -> Result<Box<dyn Tx>, StoreError> {
        let tx = self.pool.begin().await.map_err(map_sqlx)?;
        Ok(Box::new(PgTx { tx }))
    }
}

/// One open transaction. Dropping it without committing rolls it back.
#[derive(Debug)]
pub struct PgTx {
    pub(crate) tx: Transaction<'static, Postgres>,
}

#[async_trait]
impl Tx for PgTx {
    fn users(&mut self) -> &mut dyn UserRepo {
        self
    }
    fn invitations(&mut self) -> &mut dyn InvitationRepo {
        self
    }
    fn resets(&mut self) -> &mut dyn PasswordResetRepo {
        self
    }
    fn sessions(&mut self) -> &mut dyn SessionRepo {
        self
    }
    fn mfa(&mut self) -> &mut dyn MfaRepo {
        self
    }
    fn guards(&mut self) -> &mut dyn GuardRepo {
        self
    }
    fn settings(&mut self) -> &mut dyn SettingsRepo {
        self
    }
    fn audit(&mut self) -> &mut dyn AuditRepo {
        self
    }
    fn chat(&mut self) -> &mut dyn ChatRepo {
        self
    }
    fn outbox(&mut self) -> &mut dyn OutboxRepo {
        self
    }
    fn jobs(&mut self) -> &mut dyn JobQueue {
        self
    }
    fn providers(&mut self) -> &mut dyn ProviderRepo {
        self
    }
    fn defaults(&mut self) -> &mut dyn DefaultsRepo {
        self
    }
    fn search(&mut self) -> &mut dyn SearchRepo {
        self
    }
    fn connectors(&mut self) -> &mut dyn ConnectorRepo {
        self
    }
    fn policies(&mut self) -> &mut dyn PolicyRepo {
        self
    }
    fn test_runs(&mut self) -> &mut dyn TestRunRepo {
        self
    }

    async fn lock_setup(&mut self) -> Result<(), StoreError> {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(SETUP_LOCK_KEY)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn lock_admin_changes(&mut self) -> Result<(), StoreError> {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(ADMIN_LOCK_KEY)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn commit(self: Box<Self>) -> Result<(), StoreError> {
        self.tx.commit().await.map_err(map_sqlx)
    }
}
