//! Last-admin recovery through the operations CLI (BR-4.7, C27).

use crate::deps::App;
use crate::error::AppError;
use crate::password::ResetLink;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::email::Email;
use tandem_core::ids::PasswordResetId;
use tandem_core::reset::PasswordReset;
use tandem_core::role::Role;
use tandem_core::session::EndReason;
use tandem_core::token::generate_token;

impl App {
    /// Issue a reset link (that also resets MFA) for the only active admin, for the operations
    /// CLI. It refuses unless the account is an active admin and the sole one: with several
    /// admins, another admin issues the link from the interface instead.
    ///
    /// The reset row names the admin as its own issuer because there is no signed-in actor.
    ///
    /// # Errors
    /// [`AppError::NotFound`] when the email is not an active admin; [`AppError::Validation`]
    /// when there is more than one active admin.
    pub async fn issue_admin_reset_for_cli(&self, email: &str) -> Result<ResetLink, AppError> {
        let email = Email::parse(email)
            .map_err(|_| AppError::validation("email", "Enter a valid email address"))?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let user = tx
            .users()
            .find_by_email(&email)
            .await?
            .filter(|user| user.role == Role::Admin && user.can_sign_in())
            .ok_or(AppError::NotFound)?;
        tx.lock_admin_changes().await?;
        if tx.users().count_active_admins().await? != 1 {
            return Err(AppError::validation(
                "email",
                "More than one admin is active: ask another admin to issue the link",
            ));
        }
        tx.resets().invalidate_unused_for_user(user.id, now).await?;
        tx.sessions()
            .end_all_for_user(user.id, EndReason::ResetIssued, now, None)
            .await?;
        let (token, token_hash) = generate_token(&*self.rng)?;
        let reset = PasswordReset::new(
            PasswordResetId::generate(&*self.clock, &*self.rng)?,
            user.id,
            token_hash,
            user.id,
            true,
            now,
        );
        tx.resets().insert(&reset).await?;
        let event = tandem_core::audit::AuditEvent::new(
            AuditEventType::AuthPasswordResetIssued,
            AuditOutcome::Success,
            now,
        )
        .actor_label("operations CLI")
        .source(Some("cli".to_owned()))
        .object("user", user.id)
        .detail("reset_mfa", "true")
        .detail("via", "cli");
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(ResetLink {
            token,
            expires_at: reset.expires_at,
            user_display_name: user.display_name.as_str().to_owned(),
        })
    }
}
