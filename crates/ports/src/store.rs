//! The transaction boundary. Application services open a transaction, work through the
//! repositories it exposes, and commit. Dropping a transaction without committing rolls it back.

use crate::config_repos::{
    ConnectorRepo, DefaultsRepo, PolicyRepo, ProviderRepo, SearchRepo, TestRunRepo,
};
use crate::error::StoreError;
use crate::repos::{
    AuditRepo, ChatRepo, GuardRepo, InvitationRepo, JobQueue, MfaRepo, OutboxRepo,
    PasswordResetRepo, SessionRepo, SettingsRepo, UserRepo,
};
use async_trait::async_trait;

/// Opens transactions.
#[async_trait]
pub trait Store: Send + Sync {
    /// Begin a transaction.
    ///
    /// # Errors
    /// [`StoreError::Unavailable`] when no connection can be obtained in time.
    async fn begin(&self) -> Result<Box<dyn Tx>, StoreError>;
}

/// One transaction. All repositories see the same snapshot and commit or roll back together.
#[async_trait]
pub trait Tx: Send {
    /// Users.
    fn users(&mut self) -> &mut dyn UserRepo;
    /// Invitations.
    fn invitations(&mut self) -> &mut dyn InvitationRepo;
    /// Password reset links.
    fn resets(&mut self) -> &mut dyn PasswordResetRepo;
    /// Sign-ins.
    fn sessions(&mut self) -> &mut dyn SessionRepo;
    /// MFA credentials and recovery codes.
    fn mfa(&mut self) -> &mut dyn MfaRepo;
    /// Sign-in guards.
    fn guards(&mut self) -> &mut dyn GuardRepo;
    /// Instance settings.
    fn settings(&mut self) -> &mut dyn SettingsRepo;
    /// The audit log (append and read only).
    fn audit(&mut self) -> &mut dyn AuditRepo;
    /// Skeleton chat.
    fn chat(&mut self) -> &mut dyn ChatRepo;
    /// The outbox.
    fn outbox(&mut self) -> &mut dyn OutboxRepo;
    /// The job queue.
    fn jobs(&mut self) -> &mut dyn JobQueue;
    /// Model providers and their models.
    fn providers(&mut self) -> &mut dyn ProviderRepo;
    /// Default models.
    fn defaults(&mut self) -> &mut dyn DefaultsRepo;
    /// Search settings.
    fn search(&mut self) -> &mut dyn SearchRepo;
    /// Connectors.
    fn connectors(&mut self) -> &mut dyn ConnectorRepo;
    /// Sandbox policies.
    fn policies(&mut self) -> &mut dyn PolicyRepo;
    /// Configuration test runs.
    fn test_runs(&mut self) -> &mut dyn TestRunRepo;

    /// Serialise first-run setup so two requests cannot both create the first admin (BR-1.1).
    async fn lock_setup(&mut self) -> Result<(), StoreError>;

    /// Serialise changes that depend on the number of active admins (BR-7.2).
    async fn lock_admin_changes(&mut self) -> Result<(), StoreError>;

    /// Commit.
    async fn commit(self: Box<Self>) -> Result<(), StoreError>;
}
