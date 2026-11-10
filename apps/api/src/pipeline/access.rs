//! Authorisation classes, the typed extractors and the route guard (LC-9, SE-9).

use crate::error::ApiError;
use crate::pipeline::SessionInfo;
use axum::extract::{FromRequestParts, Request};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tandem_core::role::Role;
use tandem_core::session::Scope;

/// Who may call a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Anyone, signed in or not.
    Public,
    /// Any valid sign-in, including one that may only enrol MFA.
    AnyUser,
    /// A sign-in with full scope.
    FullScopeUser,
    /// An admin with a full-scope sign-in. Others are told the route does not exist (BR-9.2).
    AdminUser,
}

/// The decision for a session against a class.
///
/// # Errors
/// The [`ApiError`] to answer with.
pub fn check(access: AccessKind, session: Option<&SessionInfo>) -> Result<(), ApiError> {
    let session = match (access, session) {
        (AccessKind::Public, _) => return Ok(()),
        (_, None) => return Err(ApiError::unauthenticated()),
        (_, Some(session)) => session,
    };
    match access {
        AccessKind::Public | AccessKind::AnyUser => Ok(()),
        AccessKind::FullScopeUser => {
            if session.actor.scope == Scope::Full {
                Ok(())
            } else {
                Err(ApiError::enrolment_required())
            }
        }
        AccessKind::AdminUser => {
            if session.actor.role != Role::Admin {
                Err(ApiError::not_found())
            } else if session.actor.scope != Scope::Full {
                Err(ApiError::enrolment_required())
            } else {
                Ok(())
            }
        }
    }
}

/// The route-level guard: enforces the class the route was registered with, whatever the handler
/// does.
pub async fn guard(
    axum::extract::State(access): axum::extract::State<AccessKind>,
    request: Request,
    next: Next,
) -> Response {
    let session = request.extensions().get::<SessionInfo>();
    match check(access, session) {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

/// A class of caller a handler can declare in its signature.
pub trait Access: Sized {
    /// The class.
    const KIND: AccessKind;
    /// Wrap the session.
    fn new(session: SessionInfo) -> Self;
    /// The session.
    fn session(&self) -> &SessionInfo;
}

macro_rules! access_extractor {
    ($(#[$doc:meta])* $name:ident, $kind:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone)]
        pub struct $name(pub SessionInfo);

        impl Access for $name {
            const KIND: AccessKind = $kind;
            fn new(session: SessionInfo) -> Self {
                Self(session)
            }
            fn session(&self) -> &SessionInfo {
                &self.0
            }
        }

        impl<S: Send + Sync> FromRequestParts<S> for $name {
            type Rejection = ApiError;

            async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
                let session = parts.extensions.get::<SessionInfo>().cloned();
                check($kind, session.as_ref())?;
                session.map(Self).ok_or_else(ApiError::unauthenticated)
            }
        }
    };
}

access_extractor!(
    /// Any valid sign-in.
    AnyUser,
    AccessKind::AnyUser
);
access_extractor!(
    /// A full-scope sign-in.
    FullScopeUser,
    AccessKind::FullScopeUser
);
access_extractor!(
    /// An admin with a full-scope sign-in.
    AdminUser,
    AccessKind::AdminUser
);
