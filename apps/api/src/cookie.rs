//! The session cookie (BR-8.1): `__Host-` prefixed, Secure, HttpOnly, SameSite=Lax, Path=/.

use axum::http::{HeaderMap, HeaderValue, header};

/// The cookie name. The `__Host-` prefix forbids a Domain attribute and requires Secure and
/// Path=/, so a sibling subdomain cannot plant a session.
pub const SESSION_COOKIE: &str = "__Host-tandem_session";

/// The value of the session cookie, if the request carries one.
pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

/// `Set-Cookie` for a new session.
pub fn set_session(token: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Lax"
    ))
    .ok()
}

/// `Set-Cookie` that removes the session cookie.
pub fn clear_session() -> HeaderValue {
    HeaderValue::from_static(
        "__Host-tandem_session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0",
    )
}
