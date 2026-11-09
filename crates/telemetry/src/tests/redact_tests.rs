use crate::redact::is_sensitive;

#[test]
fn names_that_carry_secrets_are_sensitive() {
    for name in [
        "password",
        "new_password",
        "current-password",
        "passwords",
        "token",
        "csrf_token",
        "session_token",
        "metrics_token",
        "secret",
        "client_secret",
        "code",
        "recovery_code",
        "totp_code",
        "cookie",
        "set_cookie",
        "authorization",
        "Authorization",
        "database_credentials",
        "api_key_secret",
        "email",
        "user_email",
        "otp",
    ] {
        assert!(is_sensitive(name), "{name} must be redacted");
    }
}

#[test]
fn ordinary_names_are_not_sensitive() {
    for name in [
        "user_id",
        "request_id",
        "job_id",
        "route",
        "status",
        "status_code",
        "exit_code",
        "decoded",
        "encoder",
        "duration_ms",
        "key_id",
        "msg",
        "method",
        "component",
    ] {
        assert!(!is_sensitive(name), "{name} must be left alone");
    }
}
