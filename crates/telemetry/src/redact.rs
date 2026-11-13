//! Which log field names must never carry a value into the logs (SE-4).

/// What a redacted value is replaced with.
pub const REDACTED: &str = "[redacted]";

/// Field-name segments that mark a value as sensitive when they appear anywhere in the segment.
const SENSITIVE_SUBSTRINGS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "cookie",
    "authorization",
    "credential",
    "apikey",
];

/// Segments that are sensitive only as a whole word (`code` would otherwise catch `decode`).
const SENSITIVE_WORDS: &[&str] = &["code", "email", "csrf", "otp", "totp", "pwd", "key"];

/// Whole names that look sensitive but are not (`key` alone catches `api_key` and `private_key`; the
/// key identifiers and the "a key is stored" timestamp that provider settings log are safe).
const SAFE_NAMES: &[&str] = &[
    "status_code",
    "exit_code",
    "error_code",
    "reason_code",
    "key_id",
    "key_saved_at",
];

/// Whether a field called `name` must have its value replaced.
pub fn is_sensitive(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    if SAFE_NAMES.contains(&lowered.as_str()) {
        return false;
    }
    lowered
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            SENSITIVE_WORDS.contains(&segment)
                || SENSITIVE_SUBSTRINGS
                    .iter()
                    .any(|marker| segment.contains(marker))
        })
}
