//! Envelope encryption for secrets at rest (SE-2, NFR design 5.3).
//!
//! AES-256-GCM with a random 96-bit nonce per value. A sealed value records the id of the key that
//! sealed it, so the key can be rotated: the key file may hold one `current` key and any number of
//! `decrypt-only` keys. The associated data binds each value to its purpose and row, so a sealed
//! value cannot be moved to another place.

mod keys;
mod vault;

pub use keys::{KeyFileError, KeyStatus};
pub use vault::EnvelopeVault;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-vault";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
