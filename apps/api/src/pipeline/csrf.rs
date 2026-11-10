//! CSRF token and the Origin check (SE-6). The token is derived from the session cookie, so
//! nothing extra is stored and a site that cannot read the cookie cannot compute it.

use tandem_core::token::hash_token;

/// The CSRF token for a session cookie value.
pub fn csrf_token_for(session_token: &str) -> String {
    hash_token(&format!("csrf-v1:{session_token}"))
        .as_str()
        .to_owned()
}

/// Compare two byte strings without stopping at the first difference.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// Whether `method` changes state and so needs the CSRF and Origin checks.
pub fn is_unsafe_method(method: &axum::http::Method) -> bool {
    !matches!(
        *method,
        axum::http::Method::GET
            | axum::http::Method::HEAD
            | axum::http::Method::OPTIONS
            | axum::http::Method::TRACE
    )
}
