//! Sign-in, sign-out and the current user (W4, W11; BR-3, BR-4).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use crate::mfa::MfaStatus;
use crate::sessions::{NewSession, scope_for};
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::email::Email;
use tandem_core::mfa::MfaCredential;
use tandem_core::permissions::Action;
use tandem_core::session::{EndReason, Scope};
use tandem_core::throttle::{FailureOutcome, GuardDecision, GuardState};
use tandem_core::token::{TokenHash, hash_recovery_code, hash_token};
use tandem_core::user::User;
use tandem_ports::Tx;
use time::OffsetDateTime;

/// A second factor supplied with the sign-in.
#[derive(Debug, Clone)]
pub enum SecondFactor {
    /// A 6-digit authenticator code.
    Totp(String),
    /// A recovery code.
    Recovery(String),
}

/// Sign-in input.
#[derive(Debug, Clone)]
pub struct SignInInput {
    /// The email as typed.
    pub email: String,
    /// The password.
    pub password: String,
    /// The second factor, when the account has MFA and the client has asked for it.
    pub second_factor: Option<SecondFactor>,
}

/// A successful step of sign-in.
#[derive(Debug, Clone)]
pub enum SignInOutcome {
    /// Signed in.
    Success(Box<NewSession>),
    /// The password was right but a second factor is needed; resubmit with one.
    MfaRequired,
}

/// The signed-in user as the client sees them.
#[derive(Debug, Clone)]
pub struct Me {
    /// The user.
    pub user: User,
    /// The effective scope of this sign-in.
    pub scope: Scope,
    /// MFA state.
    pub mfa: MfaStatus,
}

enum Attempt {
    Failed,
    NeedsSecondFactor,
    Verified { mfa_confirmed: bool, rehash: bool },
}

impl App {
    /// Sign in with email, password and (when enrolled) a second factor.
    ///
    /// Every failure, for known and unknown accounts alike, is [`AppError::InvalidCredentials`];
    /// delays, locks and rate limits are [`AppError::Refused`] (BR-3.5, BR-3.6).
    ///
    /// # Errors
    /// See above; [`AppError::Busy`] when the host is hashing too many passwords.
    pub async fn sign_in(
        &self,
        ctx: &RequestContext,
        input: SignInInput,
    ) -> Result<SignInOutcome, AppError> {
        let now = self.clock.now();
        let guard_key = hash_token(&format!(
            "signin-guard:{}",
            input.email.trim().to_lowercase()
        ));
        let mut tx = self.store.begin().await?;
        let user = match Email::parse(&input.email) {
            Ok(email) => tx.users().find_by_email(&email).await?,
            Err(_) => None,
        };
        let mut guard = match &user {
            Some(user) => tx.guards().get_account_guard_for_update(user.id).await?,
            None => tx.guards().get_unknown_guard_for_update(&guard_key).await?,
        };
        match guard.decision(now) {
            GuardDecision::Proceed => {}
            GuardDecision::WaitUntil(retry_at) | GuardDecision::LockedUntil(retry_at) => {
                return Err(AppError::Refused { retry_at });
            }
        }

        let active = user.as_ref().filter(|user| user.can_sign_in());
        match self
            .check_credentials(tx.as_mut(), ctx, active, &input, now)
            .await?
        {
            Attempt::NeedsSecondFactor => Ok(SignInOutcome::MfaRequired),
            Attempt::Failed => {
                self.record_failure(tx.as_mut(), ctx, user.as_ref(), &mut guard, &guard_key, now)
                    .await?;
                tx.commit().await?;
                Err(AppError::InvalidCredentials)
            }
            Attempt::Verified {
                mfa_confirmed,
                rehash,
            } => {
                let Some(mut user) = user else {
                    return Err(AppError::Internal("verified sign-in without a user".into()));
                };
                if rehash {
                    // Best effort: a busy hasher must not fail a correct sign-in.
                    if let Ok(new_hash) = self.hasher.hash(&input.password).await {
                        user.password_hash = new_hash;
                    }
                }
                guard.record_success();
                tx.guards().put_account_guard(user.id, &guard).await?;
                let scope = scope_for(user.role, mfa_confirmed);
                let session = self.start_session(tx.as_mut(), &user, scope, ctx).await?;
                user.record_sign_in(now);
                tx.users().update(&user).await?;
                let event = self
                    .audit_event(
                        ctx,
                        AuditEventType::AuthSignInSuccess,
                        AuditOutcome::Success,
                    )
                    .actor(user.id, user.display_name.as_str())
                    .object("user", user.id)
                    .detail("scope", scope.as_str());
                self.append_audit(tx.as_mut(), event).await?;
                tx.commit().await?;
                Ok(SignInOutcome::Success(Box::new(session)))
            }
        }
    }

    async fn check_credentials(
        &self,
        tx: &mut dyn Tx,
        ctx: &RequestContext,
        active: Option<&User>,
        input: &SignInInput,
        now: OffsetDateTime,
    ) -> Result<Attempt, AppError> {
        let Some(user) = active else {
            self.hasher.dummy_verify().await?;
            return Ok(Attempt::Failed);
        };
        let verification = self
            .hasher
            .verify(&input.password, &user.password_hash)
            .await?;
        if !verification.matches {
            return Ok(Attempt::Failed);
        }
        let credential = tx
            .mfa()
            .get_credential(user.id)
            .await?
            .filter(MfaCredential::is_confirmed);
        let Some(credential) = credential else {
            return Ok(Attempt::Verified {
                mfa_confirmed: false,
                rehash: verification.needs_rehash,
            });
        };
        let Some(factor) = &input.second_factor else {
            return Ok(Attempt::NeedsSecondFactor);
        };
        let accepted = match factor {
            SecondFactor::Totp(code) => self.accept_totp(tx, user, credential, code, now).await?,
            SecondFactor::Recovery(code) => {
                self.accept_recovery_code(tx, ctx, user, code, now).await?
            }
        };
        if accepted {
            Ok(Attempt::Verified {
                mfa_confirmed: true,
                rehash: verification.needs_rehash,
            })
        } else {
            Ok(Attempt::Failed)
        }
    }

    pub(crate) async fn accept_totp(
        &self,
        tx: &mut dyn Tx,
        user: &User,
        mut credential: MfaCredential,
        code: &str,
        now: OffsetDateTime,
    ) -> Result<bool, AppError> {
        let secret = self.vault.open(
            "mfa_secret",
            &user.id.to_string(),
            &credential.secret_sealed,
        )?;
        let Some(step) = self.totp.verify(&secret, code, now) else {
            return Ok(false);
        };
        if credential.accept_step(step).is_err() {
            return Ok(false);
        }
        tx.mfa().upsert_credential(&credential).await?;
        Ok(true)
    }

    async fn accept_recovery_code(
        &self,
        tx: &mut dyn Tx,
        ctx: &RequestContext,
        user: &User,
        code: &str,
        now: OffsetDateTime,
    ) -> Result<bool, AppError> {
        let hash = hash_recovery_code(code);
        let Some(mut recovery) = tx.mfa().find_recovery_code(user.id, &hash).await? else {
            return Ok(false);
        };
        if recovery.use_code(now).is_err() {
            return Ok(false);
        }
        tx.mfa().update_recovery_code(&recovery).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::AuthMfaRecoveryUsed,
                AuditOutcome::Success,
            )
            .actor(user.id, user.display_name.as_str())
            .object("user", user.id);
        self.append_audit(tx, event).await?;
        Ok(true)
    }

    async fn record_failure(
        &self,
        tx: &mut dyn Tx,
        ctx: &RequestContext,
        user: Option<&User>,
        guard: &mut GuardState,
        guard_key: &TokenHash,
        now: OffsetDateTime,
    ) -> Result<(), AppError> {
        let outcome = guard.record_failure(now);
        match user {
            Some(user) => {
                tx.guards().put_account_guard(user.id, guard).await?;
                // Failures are written only for existing accounts, so mistyped input is never
                // stored (BR-11.3).
                let failure = self
                    .audit_event(
                        ctx,
                        AuditEventType::AuthSignInFailure,
                        AuditOutcome::Failure,
                    )
                    .actor_label(user.display_name.as_str())
                    .object("user", user.id)
                    .detail("failures_in_window", guard.failure_count.to_string());
                self.append_audit(tx, failure).await?;
                if let FailureOutcome::Locked(until) = outcome {
                    let locked = self
                        .audit_event(ctx, AuditEventType::AuthSignInLocked, AuditOutcome::Denied)
                        .actor_label(user.display_name.as_str())
                        .object("user", user.id)
                        .detail("locked_until", until.to_string());
                    self.append_audit(tx, locked).await?;
                }
            }
            None => tx.guards().put_unknown_guard(guard_key, guard, now).await?,
        }
        Ok(())
    }

    /// End the current sign-in.
    ///
    /// # Errors
    /// [`AppError::Unauthenticated`] when nobody is signed in.
    pub async fn sign_out(&self, ctx: &RequestContext) -> Result<(), AppError> {
        self.authorize(ctx, Action::SignOut).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        if let Some(mut session) = tx.sessions().get(actor.session_id).await? {
            session.end(EndReason::SignedOut, self.clock.now());
            tx.sessions().update(&session).await?;
        }
        let event = self
            .audit_event(ctx, AuditEventType::AuthSignOut, AuditOutcome::Success)
            .object("user", actor.user_id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    /// The current user, scope and MFA state.
    ///
    /// # Errors
    /// [`AppError::Unauthenticated`] when nobody is signed in.
    pub async fn current_user(&self, ctx: &RequestContext) -> Result<Me, AppError> {
        self.authorize(ctx, Action::CurrentUser).await?;
        let actor = ctx.actor()?;
        let mut tx = self.store.begin().await?;
        let user = tx
            .users()
            .get(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let mfa = Self::mfa_status_in(tx.as_mut(), user.id).await?;
        Ok(Me {
            user,
            scope: actor.scope,
            mfa,
        })
    }
}
