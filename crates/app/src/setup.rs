//! First-run setup (W1, US-1.1).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use crate::sessions::{NewSession, scope_for};
use crate::validation::{fail_if_any, name_and_email};
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::ids::UserId;
use tandem_core::permissions::Action;
use tandem_core::role::{Role, UserStatus};
use tandem_core::user::{CreatedVia, User};

/// Input for the first-run setup screen.
#[derive(Debug, Clone)]
pub struct SetupInput {
    /// The admin's display name.
    pub display_name: String,
    /// The admin's email address.
    pub email: String,
    /// The admin's password.
    pub password: String,
}

impl App {
    /// Whether first-run setup is still available (no user exists, BR-1.1).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn setup_available(&self) -> Result<bool, AppError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.users().count_all().await? == 0)
    }

    /// Create the first admin and sign them in with an enrolment-only scope (BR-4.1).
    ///
    /// # Errors
    /// [`AppError::NotFound`] once any user exists; [`AppError::Validation`] for bad input.
    pub async fn first_run_setup(
        &self,
        ctx: &RequestContext,
        input: SetupInput,
    ) -> Result<NewSession, AppError> {
        self.authorize(ctx, Action::FirstRunSetup).await?;

        let mut issues = Vec::new();
        let (name, email) = name_and_email(&input.display_name, &input.email, &mut issues);
        issues.extend(self.password_issues(&input.password, false, None).await?);
        fail_if_any(issues)?;
        let (Some(display_name), Some(email)) = (name, email) else {
            return Err(AppError::Internal("validated input was missing".into()));
        };
        let password_hash = self.hasher.hash(&input.password).await?;

        let mut tx = self.store.begin().await?;
        tx.lock_setup().await?;
        if tx.users().count_all().await? > 0 {
            return Err(AppError::NotFound);
        }
        let now = self.clock.now();
        let user = User {
            id: UserId::generate(&*self.clock, &*self.rng)?,
            email,
            display_name,
            role: Role::Admin,
            status: UserStatus::Active,
            password_hash,
            password_changed_at: now,
            created_at: now,
            created_via: CreatedVia::FirstRunSetup,
            deactivated_at: None,
            deactivated_by: None,
            last_sign_in_at: None,
        };
        tx.users().insert(&user).await?;
        let event = self
            .audit_event(ctx, AuditEventType::AuthSetup, AuditOutcome::Success)
            .actor(user.id, user.display_name.as_str())
            .object("user", user.id);
        self.append_audit(tx.as_mut(), event).await?;
        let session = self
            .start_session(tx.as_mut(), &user, scope_for(user.role, false), ctx)
            .await?;
        tx.commit().await?;
        Ok(session)
    }
}
