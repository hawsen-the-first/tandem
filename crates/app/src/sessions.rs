//! Starting and resolving sign-ins (W3, W4, W11, W14).

use crate::context::{Actor, RequestContext};
use crate::deps::App;
use crate::error::AppError;
use tandem_core::ids::AuthSessionId;
use tandem_core::role::Role;
use tandem_core::session::{AuthSession, EndReason, Scope, SessionStatus};
use tandem_core::token::{generate_token, hash_token};
use tandem_core::user::User;
use tandem_ports::Tx;

/// A sign-in that was just created. The token is shown once (as the cookie value).
#[derive(Debug, Clone)]
pub struct NewSession {
    /// The user who signed in.
    pub user: User,
    /// The cookie token (plaintext; only its hash is stored).
    pub token: String,
    /// What the sign-in may do.
    pub scope: Scope,
    /// The sign-in's identifier.
    pub session_id: AuthSessionId,
}

/// A resolved, valid sign-in for the current request.
#[derive(Debug, Clone)]
pub struct Authenticated {
    /// The caller, with the effective scope.
    pub actor: Actor,
    /// The user as stored now.
    pub user: User,
    /// Whether the user has confirmed MFA.
    pub mfa_enrolled: bool,
}

/// An admin without confirmed MFA gets an enrolment-only sign-in (BR-4.1).
pub(crate) fn scope_for(role: Role, mfa_confirmed: bool) -> Scope {
    if role == Role::Admin && !mfa_confirmed {
        Scope::MfaEnrolmentOnly
    } else {
        Scope::Full
    }
}

impl App {
    /// Create a sign-in for the user, copying the current duration settings onto it (BR-8.2).
    pub(crate) async fn start_session(
        &self,
        tx: &mut dyn Tx,
        user: &User,
        scope: Scope,
        ctx: &RequestContext,
    ) -> Result<NewSession, AppError> {
        let settings = Self::load_settings(tx).await?;
        let now = self.clock.now();
        let (token, token_hash) = generate_token(&*self.rng)?;
        let id = AuthSessionId::generate(&*self.clock, &*self.rng)?;
        let session = AuthSession {
            id,
            user_id: user.id,
            token_hash,
            created_at: now,
            last_seen_at: now,
            idle_timeout: settings.idle_timeout,
            absolute_duration: settings.absolute_duration,
            scope,
            ended_at: None,
            ended_reason: None,
            source_summary: ctx.source.clone(),
        };
        tx.sessions().insert(&session).await?;
        Ok(NewSession {
            user: user.clone(),
            token,
            scope,
            session_id: id,
        })
    }

    /// Resolve the cookie token of the current request (W11, W14). Reads the session, the user's
    /// role, status and MFA state from the database every time (BR-9.1); nothing is cached.
    ///
    /// # Errors
    /// [`AppError::Unauthenticated`] when the token is unknown, ended, expired, or its user is
    /// deactivated.
    pub async fn resolve_session(&self, token: &str) -> Result<Authenticated, AppError> {
        let now = self.clock.now();
        let hash = hash_token(token);
        let mut tx = self.store.begin().await?;
        let Some(mut session) = tx.sessions().find_by_token_hash(&hash).await? else {
            return Err(AppError::Unauthenticated);
        };
        match session.status(now) {
            SessionStatus::Active => {}
            SessionStatus::IdleExpired => {
                return self.end_expired(tx, session, EndReason::IdleTimeout).await;
            }
            SessionStatus::AbsoluteExpired => {
                return self
                    .end_expired(tx, session, EndReason::AbsoluteTimeout)
                    .await;
            }
            SessionStatus::Ended(_) => return Err(AppError::Unauthenticated),
        }
        let Some(user) = tx.users().get(session.user_id).await? else {
            return Err(AppError::Unauthenticated);
        };
        if !user.can_sign_in() {
            return Err(AppError::Unauthenticated);
        }
        let mfa_enrolled = tx
            .mfa()
            .get_credential(user.id)
            .await?
            .is_some_and(|credential| credential.is_confirmed());
        let scope = if session.scope == Scope::Full {
            scope_for(user.role, mfa_enrolled)
        } else {
            session.scope
        };
        if session.needs_touch(now) {
            session.touch(now);
            tx.sessions().update(&session).await?;
            tx.commit().await?;
        }
        Ok(Authenticated {
            actor: Actor {
                user_id: user.id,
                display_name: user.display_name.as_str().to_owned(),
                role: user.role,
                scope,
                session_id: session.id,
            },
            user,
            mfa_enrolled,
        })
    }

    async fn end_expired(
        &self,
        mut tx: Box<dyn Tx>,
        mut session: AuthSession,
        reason: EndReason,
    ) -> Result<Authenticated, AppError> {
        session.end(reason, self.clock.now());
        tx.sessions().update(&session).await?;
        tx.commit().await?;
        Err(AppError::Unauthenticated)
    }
}
