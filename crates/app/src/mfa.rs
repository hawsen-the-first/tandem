//! MFA enrolment, recovery codes and removal (W5, W6; BR-4).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::ids::{RecoveryCodeId, UserId};
use tandem_core::mfa::{
    MfaCredential, RECOVERY_CODE_COUNT, RecoveryCode, should_remind_to_regenerate,
};
use tandem_core::permissions::Action;
use tandem_core::role::Role;
use tandem_core::token::{generate_recovery_code, hash_recovery_code};
use tandem_ports::Tx;

/// What the user needs to set up their authenticator app.
#[derive(Debug, Clone)]
pub struct Enrolment {
    /// The secret as base32 text, for manual entry.
    pub secret_base32: String,
    /// The `otpauth://` URI to render as a QR code.
    pub otpauth_uri: String,
}

/// A user's MFA state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MfaStatus {
    /// MFA is enrolled and confirmed.
    pub enrolled: bool,
    /// Unused recovery codes left.
    pub recovery_codes_remaining: usize,
    /// Suggest generating a new set (3 or fewer left).
    pub remind_regenerate: bool,
}

impl App {
    pub(crate) async fn mfa_status_in(
        tx: &mut dyn Tx,
        user_id: UserId,
    ) -> Result<MfaStatus, AppError> {
        let enrolled = tx
            .mfa()
            .get_credential(user_id)
            .await?
            .is_some_and(|credential| credential.is_confirmed());
        let remaining = if enrolled {
            tx.mfa().count_unused_recovery_codes(user_id).await?
        } else {
            0
        };
        Ok(MfaStatus {
            enrolled,
            recovery_codes_remaining: remaining,
            remind_regenerate: enrolled && should_remind_to_regenerate(remaining),
        })
    }

    /// The caller's MFA state.
    ///
    /// # Errors
    /// [`AppError::Unauthenticated`] when nobody is signed in.
    pub async fn mfa_status(&self, ctx: &RequestContext) -> Result<MfaStatus, AppError> {
        self.authorize(ctx, Action::ManageOwnMfa).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        Self::mfa_status_in(tx.as_mut(), actor.user_id).await
    }

    /// Begin enrolment: create an unconfirmed credential and return what the authenticator needs.
    ///
    /// # Errors
    /// [`AppError::Validation`] when MFA is already enrolled.
    pub async fn mfa_start_enrolment(&self, ctx: &RequestContext) -> Result<Enrolment, AppError> {
        self.authorize(ctx, Action::ManageOwnMfa).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        let user = tx
            .users()
            .get(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if tx
            .mfa()
            .get_credential(user.id)
            .await?
            .is_some_and(|c| c.is_confirmed())
        {
            return Err(AppError::validation("mfa", "MFA is already set up"));
        }
        let secret = self.totp.generate_secret()?;
        let sealed = self
            .vault
            .seal("mfa_secret", &user.id.to_string(), &secret)?;
        tx.mfa()
            .upsert_credential(&MfaCredential::new(user.id, sealed))
            .await?;
        tx.commit().await?;
        Ok(Enrolment {
            secret_base32: self.totp.secret_base32(&secret),
            otpauth_uri: self
                .totp
                .otpauth_uri(&secret, user.email.as_str(), &self.issuer),
        })
    }

    /// Confirm enrolment with a valid code. Returns the recovery codes, to be shown once. An
    /// enrolment-only sign-in is upgraded to full scope.
    ///
    /// # Errors
    /// [`AppError::Validation`] when no enrolment is in progress or the code is wrong.
    pub async fn mfa_confirm_enrolment(
        &self,
        ctx: &RequestContext,
        code: &str,
    ) -> Result<Vec<String>, AppError> {
        self.authorize(ctx, Action::ManageOwnMfa).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut credential = tx
            .mfa()
            .get_credential(actor.user_id)
            .await?
            .filter(|credential| !credential.is_confirmed())
            .ok_or_else(|| AppError::validation("mfa", "Start setting up MFA first"))?;
        let secret = self.vault.open(
            "mfa_secret",
            &actor.user_id.to_string(),
            &credential.secret_sealed,
        )?;
        let step = self.totp.verify(&secret, code, now).ok_or_else(|| {
            AppError::validation(
                "code",
                "That code is not valid. Check your authenticator and try again",
            )
        })?;
        credential
            .accept_step(step)
            .and_then(|()| credential.confirm(now))
            .map_err(|_| {
                AppError::validation(
                    "code",
                    "That code is not valid. Check your authenticator and try again",
                )
            })?;
        tx.mfa().upsert_credential(&credential).await?;
        let (plain, stored) = self.new_recovery_codes(actor.user_id)?;
        tx.mfa()
            .replace_recovery_codes(actor.user_id, &stored)
            .await?;
        if let Some(mut session) = tx.sessions().get(actor.session_id).await? {
            session.upgrade_to_full();
            tx.sessions().update(&session).await?;
        }
        let event = self
            .audit_event(ctx, AuditEventType::AuthMfaEnrolled, AuditOutcome::Success)
            .object("user", actor.user_id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(plain)
    }

    /// Replace all recovery codes with a new set, shown once.
    ///
    /// # Errors
    /// [`AppError::Validation`] when MFA is not enrolled.
    pub async fn mfa_regenerate_recovery_codes(
        &self,
        ctx: &RequestContext,
    ) -> Result<Vec<String>, AppError> {
        self.authorize(ctx, Action::ManageOwnMfa).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        let enrolled = tx
            .mfa()
            .get_credential(actor.user_id)
            .await?
            .is_some_and(|c| c.is_confirmed());
        if !enrolled {
            return Err(AppError::validation("mfa", "Set up MFA first"));
        }
        let (plain, stored) = self.new_recovery_codes(actor.user_id)?;
        tx.mfa()
            .replace_recovery_codes(actor.user_id, &stored)
            .await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::AuthMfaRecoveryRegenerated,
                AuditOutcome::Success,
            )
            .object("user", actor.user_id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(plain)
    }

    /// A member switches MFA off, confirming with their password and a current code (BR-4.6).
    /// Admins cannot.
    ///
    /// # Errors
    /// [`AppError::Forbidden`] for admins; [`AppError::Validation`] for a wrong password or code.
    pub async fn mfa_disable(
        &self,
        ctx: &RequestContext,
        password: &str,
        code: &str,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageOwnMfa).await?;
        let actor = ctx.actor()?;
        if actor.role == Role::Admin {
            return Err(AppError::Forbidden);
        }
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let user = tx
            .users()
            .get(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if !self
            .hasher
            .verify(password, &user.password_hash)
            .await?
            .matches
        {
            return Err(AppError::validation(
                "password",
                "The password is not correct",
            ));
        }
        let credential = tx
            .mfa()
            .get_credential(user.id)
            .await?
            .filter(MfaCredential::is_confirmed)
            .ok_or_else(|| AppError::validation("mfa", "MFA is not set up"))?;
        if !self
            .accept_totp(tx.as_mut(), &user, credential, code, now)
            .await?
        {
            return Err(AppError::validation("code", "That code is not valid"));
        }
        tx.mfa().delete_credential_and_codes(user.id).await?;
        let event = self
            .audit_event(ctx, AuditEventType::AuthMfaDisabled, AuditOutcome::Success)
            .object("user", user.id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    fn new_recovery_codes(
        &self,
        user_id: UserId,
    ) -> Result<(Vec<String>, Vec<RecoveryCode>), AppError> {
        let mut plain = Vec::with_capacity(RECOVERY_CODE_COUNT);
        let mut stored = Vec::with_capacity(RECOVERY_CODE_COUNT);
        while plain.len() < RECOVERY_CODE_COUNT {
            let code = generate_recovery_code(&*self.rng)?;
            let hash = hash_recovery_code(&code);
            if stored
                .iter()
                .any(|existing: &RecoveryCode| existing.code_hash == hash)
            {
                continue;
            }
            stored.push(RecoveryCode {
                id: RecoveryCodeId::generate(&*self.clock, &*self.rng)?,
                user_id,
                code_hash: hash,
                used_at: None,
            });
            plain.push(code);
        }
        Ok((plain, stored))
    }
}
