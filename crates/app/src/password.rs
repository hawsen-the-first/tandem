//! Password change and admin-issued reset links (W7, W8; BR-2, BR-6).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use crate::validation::fail_if_any;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::ids::{PasswordResetId, UserId};
use tandem_core::permissions::Action;
use tandem_core::reset::PasswordReset;
use tandem_core::session::EndReason;
use tandem_core::token::{generate_token, hash_token};
use time::OffsetDateTime;

/// A reset link just issued. The token is shown once to the issuing admin (BR-6.1).
#[derive(Debug, Clone)]
pub struct ResetLink {
    /// The link token (plaintext; only its hash is stored).
    pub token: String,
    /// When the link stops working.
    pub expires_at: OffsetDateTime,
    /// The user the link is for.
    pub user_display_name: String,
}

/// What a valid reset link shows before the new password is entered.
#[derive(Debug, Clone)]
pub struct ResetInfo {
    /// Who the reset is for.
    pub display_name: String,
}

impl App {
    /// Change the caller's own password; their other sign-ins end (BR-8.3).
    ///
    /// # Errors
    /// [`AppError::Validation`] for a wrong current password or a password that breaks the policy.
    pub async fn change_password(
        &self,
        ctx: &RequestContext,
        current: &str,
        new: &str,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ChangeOwnPassword).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut user = tx
            .users()
            .get(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if !self
            .hasher
            .verify(current, &user.password_hash)
            .await?
            .matches
        {
            return Err(AppError::validation(
                "current_password",
                "The current password is not correct",
            ));
        }
        let settings = Self::load_settings(tx.as_mut()).await?;
        fail_if_any(
            self.password_issues(new, settings.online_breach_check, Some(&user.password_hash))
                .await?,
        )?;
        let hash = self.hasher.hash(new).await?;
        user.set_password_hash(hash, now);
        tx.users().update(&user).await?;
        tx.sessions()
            .end_all_for_user(
                user.id,
                EndReason::PasswordChanged,
                now,
                Some(actor.session_id),
            )
            .await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::AuthPasswordChanged,
                AuditOutcome::Success,
            )
            .object("user", user.id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Issue a one-time reset link for a user. Their sign-ins end at once and any earlier unused
    /// link stops working (BR-6.3). `reset_mfa` also clears their MFA when the link is used.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins and unknown users.
    pub async fn issue_reset_link(
        &self,
        ctx: &RequestContext,
        target: UserId,
        reset_mfa: bool,
    ) -> Result<ResetLink, AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let user = tx.users().get(target).await?.ok_or(AppError::NotFound)?;
        tx.resets().invalidate_unused_for_user(user.id, now).await?;
        tx.sessions()
            .end_all_for_user(user.id, EndReason::ResetIssued, now, None)
            .await?;
        let (token, token_hash) = generate_token(&*self.rng)?;
        let reset = PasswordReset::new(
            PasswordResetId::generate(&*self.clock, &*self.rng)?,
            user.id,
            token_hash,
            actor.user_id,
            reset_mfa,
            now,
        );
        tx.resets().insert(&reset).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::AuthPasswordResetIssued,
                AuditOutcome::Success,
            )
            .object("user", user.id)
            .detail("reset_mfa", reset_mfa.to_string());
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(ResetLink {
            token,
            expires_at: reset.expires_at,
            user_display_name: user.display_name.as_str().to_owned(),
        })
    }

    /// Check a reset link before showing the new-password form.
    ///
    /// # Errors
    /// [`AppError::InvalidLink`] for any unusable link (BR-6.2).
    pub async fn validate_reset_link(&self, token: &str) -> Result<ResetInfo, AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let reset = tx
            .resets()
            .find_by_token_hash(&hash_token(token))
            .await?
            .ok_or(AppError::InvalidLink)?;
        reset.check_usable(now).map_err(|_| AppError::InvalidLink)?;
        let user = tx
            .users()
            .get(reset.user_id)
            .await?
            .ok_or(AppError::InvalidLink)?;
        Ok(ResetInfo {
            display_name: user.display_name.as_str().to_owned(),
        })
    }

    /// Use a reset link to set a new password (and clear MFA when the issuing admin asked).
    ///
    /// # Errors
    /// [`AppError::InvalidLink`] for any unusable link; [`AppError::Validation`] for a weak password.
    pub async fn complete_reset(
        &self,
        ctx: &RequestContext,
        token: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut reset = tx
            .resets()
            .find_by_token_hash(&hash_token(token))
            .await?
            .ok_or(AppError::InvalidLink)?;
        reset.check_usable(now).map_err(|_| AppError::InvalidLink)?;
        let mut user = tx
            .users()
            .get(reset.user_id)
            .await?
            .ok_or(AppError::InvalidLink)?;
        let settings = Self::load_settings(tx.as_mut()).await?;
        fail_if_any(
            self.password_issues(new_password, settings.online_breach_check, None)
                .await?,
        )?;
        let hash = self.hasher.hash(new_password).await?;
        user.set_password_hash(hash, now);
        tx.users().update(&user).await?;
        reset.mark_used(now).map_err(|_| AppError::InvalidLink)?;
        tx.resets().update(&reset).await?;
        if reset.reset_mfa {
            tx.mfa().delete_credential_and_codes(user.id).await?;
        }
        tx.sessions()
            .end_all_for_user(user.id, EndReason::PasswordChanged, now, None)
            .await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::AuthPasswordResetCompleted,
                AuditOutcome::Success,
            )
            .actor(user.id, user.display_name.as_str())
            .object("user", user.id)
            .detail("mfa_reset", reset.reset_mfa.to_string());
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }
}
