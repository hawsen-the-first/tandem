//! Who is making the request.

use crate::error::AppError;
use tandem_core::ids::{AuthSessionId, UserId};
use tandem_core::permissions::Principal;
use tandem_core::role::Role;
use tandem_core::session::Scope;

/// The authenticated caller, read from the database on every request (BR-9.1).
#[derive(Debug, Clone)]
pub struct Actor {
    /// The user.
    pub user_id: UserId,
    /// A snapshot of the user's name for audit entries.
    pub display_name: String,
    /// The stored role.
    pub role: Role,
    /// The effective scope of the sign-in.
    pub scope: Scope,
    /// The sign-in being used.
    pub session_id: AuthSessionId,
}

/// Request-scoped facts every use case needs.
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    /// The caller, if signed in.
    pub actor: Option<Actor>,
    /// Correlation ID for logs and audit.
    pub request_id: Option<String>,
    /// Source address and client summary.
    pub source: Option<String>,
    /// Trace context carried into outbox events and jobs so a trace follows the work.
    pub trace_context: Option<String>,
}

impl RequestContext {
    /// A context for a caller who is not signed in.
    pub fn anonymous(request_id: Option<String>, source: Option<String>) -> Self {
        Self {
            actor: None,
            request_id,
            source,
            trace_context: None,
        }
    }

    /// A context for a signed-in caller.
    pub fn authenticated(actor: Actor, request_id: Option<String>, source: Option<String>) -> Self {
        Self {
            actor: Some(actor),
            request_id,
            source,
            trace_context: None,
        }
    }

    /// The caller as a permission principal.
    pub fn principal(&self) -> Principal {
        match &self.actor {
            None => Principal::Anonymous,
            Some(actor) => Principal::Authenticated {
                role: actor.role,
                scope: actor.scope,
            },
        }
    }

    /// The signed-in caller.
    ///
    /// # Errors
    /// [`AppError::Unauthenticated`] when nobody is signed in.
    pub fn actor(&self) -> Result<&Actor, AppError> {
        self.actor.as_ref().ok_or(AppError::Unauthenticated)
    }
}

impl RequestContext {
    /// Attach the trace context of the current request.
    #[must_use]
    pub fn with_trace_context(mut self, trace_context: Option<String>) -> Self {
        self.trace_context = trace_context;
        self
    }
}
