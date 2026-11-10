//! The request pipeline, outermost first: request identity, access log and metrics, panic catcher,
//! security headers, rate limit, body limit, session resolution, CSRF and Origin checks, then the
//! route guard and the handler (NFR design section 1).

pub mod access;
pub mod csrf;
pub mod rate_limit;
pub mod source;

mod layers;

pub use access::Access;
pub use layers::{
    BODY_LIMIT_BYTES, RequestMeta, access_log, body_limit, catch_panic_response, csrf_layer,
    rate_limit_layer, request_identity, security_headers, session_layer,
};

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use source::ClientAddr;
use tandem_app::{Actor, RequestContext};
use tandem_core::user::User;

/// A resolved sign-in for this request, placed in the request extensions by the session layer.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// The caller.
    pub actor: Actor,
    /// The user as stored now.
    pub user: User,
    /// Whether MFA is confirmed.
    pub mfa_enrolled: bool,
    /// The cookie value (the CSRF token is derived from it).
    pub token: String,
}

/// The request context handlers pass to application services: actor (if any), request ID, source
/// address and trace context.
#[derive(Debug, Clone)]
pub struct Ctx(pub RequestContext);

impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let meta = parts.extensions.get::<RequestMeta>().cloned();
        let client = parts.extensions.get::<ClientAddr>().copied();
        let session = parts.extensions.get::<SessionInfo>();
        let request_id = meta.as_ref().map(|meta| meta.request_id.clone());
        let source = client.map(|client| client.ip.to_string());
        let context = match session {
            Some(session) => {
                RequestContext::authenticated(session.actor.clone(), request_id, source)
            }
            None => RequestContext::anonymous(request_id, source),
        };
        Ok(Self(context.with_trace_context(
            meta.and_then(|meta| meta.traceparent),
        )))
    }
}
