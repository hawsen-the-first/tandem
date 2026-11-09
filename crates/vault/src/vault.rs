//! The envelope vault.

use crate::keys::{Key, KeyFileError, KeyStatus, parse};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use std::path::Path;
use std::sync::Arc;
use tandem_ports::{Vault, VaultError};
use tandem_support::{Rng, random_array};

const FORMAT_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const CANARY_PURPOSE: &str = "vault_canary";
const CANARY_ROW: &str = "canary";
const CANARY_TEXT: &[u8] = b"tandem vault canary";

/// A vault over the keys of one key file.
pub struct EnvelopeVault {
    keys: Vec<Key>,
    rng: Arc<dyn Rng>,
}

impl std::fmt::Debug for EnvelopeVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvelopeVault")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

fn aad(purpose: &str, row_id: &str) -> Vec<u8> {
    format!("tandem-vault/v{FORMAT_VERSION}|{purpose}|{row_id}").into_bytes()
}

impl EnvelopeVault {
    /// Build a vault from key file text.
    ///
    /// # Errors
    /// [`KeyFileError`] describing the first problem (without key material).
    pub fn from_key_text(text: &str, rng: Arc<dyn Rng>) -> Result<Self, KeyFileError> {
        Ok(Self {
            keys: parse(text)?,
            rng,
        })
    }

    /// Build a vault from a key file. A file readable by group or others is allowed (container
    /// secrets are often mounted that way) but logged as a warning.
    ///
    /// # Errors
    /// [`KeyFileError`].
    pub fn from_key_file(path: &Path, rng: Arc<dyn Rng>) -> Result<Self, KeyFileError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(path)
                && metadata.permissions().mode() & 0o077 != 0
            {
                tracing::warn!(
                    "the master key file is readable by other users; restrict it to the service account"
                );
            }
        }
        let text = std::fs::read_to_string(path).map_err(|_| KeyFileError::Unreadable)?;
        Self::from_key_text(&text, rng)
    }

    fn current(&self) -> &Key {
        // `parse` guarantees exactly one current key.
        self.keys
            .iter()
            .find(|key| key.status == KeyStatus::Current)
            .unwrap_or(&self.keys[0])
    }

    /// The id of the key that seals new values.
    pub fn current_key_id(&self) -> &str {
        &self.current().id
    }

    /// The ids of all configured keys.
    pub fn key_ids(&self) -> Vec<&str> {
        self.keys.iter().map(|key| key.id.as_str()).collect()
    }

    fn cipher(key: &Key) -> Result<Aes256Gcm, VaultError> {
        Aes256Gcm::new_from_slice(key.bytes.as_slice()).map_err(|_| VaultError::Seal)
    }

    /// Re-seal a value under the current key. Returns `None` when it is already sealed by it.
    ///
    /// # Errors
    /// [`VaultError`] when the value cannot be opened or sealed.
    pub fn rekey(
        &self,
        purpose: &str,
        row_id: &str,
        sealed: &[u8],
    ) -> Result<Option<Vec<u8>>, VaultError> {
        let key_id = Self::key_id_of(sealed)?;
        if key_id == self.current().id {
            return Ok(None);
        }
        let plaintext = self.open(purpose, row_id, sealed)?;
        self.seal(purpose, row_id, &plaintext).map(Some)
    }

    /// Seal the canary value, stored once so start-up can prove the key works (readiness).
    ///
    /// # Errors
    /// [`VaultError::Seal`].
    pub fn seal_canary(&self) -> Result<Vec<u8>, VaultError> {
        self.seal(CANARY_PURPOSE, CANARY_ROW, CANARY_TEXT)
    }

    /// Check a stored canary.
    ///
    /// # Errors
    /// [`VaultError::Open`] when the key is wrong or the value was altered.
    pub fn verify_canary(&self, sealed: &[u8]) -> Result<(), VaultError> {
        if self.open(CANARY_PURPOSE, CANARY_ROW, sealed)? == CANARY_TEXT {
            Ok(())
        } else {
            Err(VaultError::Open)
        }
    }

    fn key_id_of(sealed: &[u8]) -> Result<&str, VaultError> {
        let (&version, rest) = sealed.split_first().ok_or(VaultError::Open)?;
        let (&id_len, rest) = rest.split_first().ok_or(VaultError::Open)?;
        if version != FORMAT_VERSION || rest.len() < usize::from(id_len) + NONCE_LEN {
            return Err(VaultError::Open);
        }
        std::str::from_utf8(&rest[..usize::from(id_len)]).map_err(|_| VaultError::Open)
    }
}

impl Vault for EnvelopeVault {
    fn seal(&self, purpose: &str, row_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let key = self.current();
        let nonce_bytes = random_array::<NONCE_LEN>(&*self.rng).map_err(|_| VaultError::Seal)?;
        let cipher = Self::cipher(key)?;
        let associated = aad(purpose, row_id);
        let ciphertext = cipher
            .encrypt(
                &Nonce::from(nonce_bytes),
                Payload {
                    msg: plaintext,
                    aad: &associated,
                },
            )
            .map_err(|_| VaultError::Seal)?;
        let id = key.id.as_bytes();
        let id_len = u8::try_from(id.len()).map_err(|_| VaultError::Seal)?;
        let mut sealed = Vec::with_capacity(2 + id.len() + NONCE_LEN + ciphertext.len());
        sealed.push(FORMAT_VERSION);
        sealed.push(id_len);
        sealed.extend_from_slice(id);
        sealed.extend_from_slice(&nonce_bytes);
        sealed.extend_from_slice(&ciphertext);
        Ok(sealed)
    }

    fn open(&self, purpose: &str, row_id: &str, sealed: &[u8]) -> Result<Vec<u8>, VaultError> {
        let key_id = Self::key_id_of(sealed)?;
        let key = self
            .keys
            .iter()
            .find(|key| key.id == key_id)
            .ok_or(VaultError::Open)?;
        let header = 2 + key_id.len();
        let nonce_bytes: [u8; NONCE_LEN] = sealed[header..header + NONCE_LEN]
            .try_into()
            .map_err(|_| VaultError::Open)?;
        let associated = aad(purpose, row_id);
        Self::cipher(key)
            .map_err(|_| VaultError::Open)?
            .decrypt(
                &Nonce::from(nonce_bytes),
                Payload {
                    msg: &sealed[header + NONCE_LEN..],
                    aad: &associated,
                },
            )
            .map_err(|_| VaultError::Open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use tandem_support::{FakeRng, SystemRng};

    fn key(byte: u8) -> String {
        STANDARD.encode([byte; 32])
    }

    fn vault(text: &str) -> EnvelopeVault {
        EnvelopeVault::from_key_text(text, Arc::new(SystemRng)).unwrap()
    }

    fn one_key() -> EnvelopeVault {
        vault(&format!("k1 current {}", key(1)))
    }

    #[test]
    fn se_2_a_sealed_value_opens_with_the_same_purpose_and_row() {
        let v = one_key();
        let sealed = v.seal("mfa_secret", "user-1", b"top secret").unwrap();
        assert_eq!(
            v.open("mfa_secret", "user-1", &sealed).unwrap(),
            b"top secret"
        );
        assert!(
            !sealed.windows(10).any(|w| w == b"top secret"),
            "the plaintext is not visible"
        );
    }

    #[test]
    fn se_2_every_seal_uses_a_fresh_nonce() {
        let v = one_key();
        assert_ne!(
            v.seal("p", "r", b"same").unwrap(),
            v.seal("p", "r", b"same").unwrap()
        );
    }

    #[test]
    fn se_2_a_value_cannot_be_moved_to_another_purpose_or_row() {
        let v = one_key();
        let sealed = v.seal("mfa_secret", "user-1", b"x").unwrap();
        assert_eq!(
            v.open("mfa_secret", "user-2", &sealed),
            Err(VaultError::Open)
        );
        assert_eq!(
            v.open("connector_token", "user-1", &sealed),
            Err(VaultError::Open)
        );
    }

    #[test]
    fn se_2_tampering_is_detected_anywhere_in_the_value() {
        let v = one_key();
        let sealed = v.seal("p", "r", b"payload").unwrap();
        for i in 0..sealed.len() {
            let mut altered = sealed.clone();
            altered[i] ^= 0x01;
            assert!(v.open("p", "r", &altered).is_err(), "byte {i}");
        }
        assert!(v.open("p", "r", &sealed[..sealed.len() - 1]).is_err());
        assert!(v.open("p", "r", &[]).is_err());
        assert!(v.open("p", "r", &[1]).is_err());
    }

    #[test]
    fn se_2_a_different_key_cannot_open_the_value() {
        let sealed = one_key().seal("p", "r", b"x").unwrap();
        let other = vault(&format!("k1 current {}", key(9)));
        assert_eq!(other.open("p", "r", &sealed), Err(VaultError::Open));
        let unknown_id = vault(&format!("zzz current {}", key(1)));
        assert_eq!(unknown_id.open("p", "r", &sealed), Err(VaultError::Open));
    }

    #[test]
    fn se_2_keys_rotate_without_losing_values() {
        let old = one_key();
        let sealed = old.seal("p", "r", b"keep me").unwrap();
        let rotated = vault(&format!(
            "k1 decrypt-only {}\nk2 current {}",
            key(1),
            key(2)
        ));
        assert_eq!(rotated.current_key_id(), "k2");
        assert_eq!(rotated.key_ids(), vec!["k1", "k2"]);
        assert_eq!(
            rotated.open("p", "r", &sealed).unwrap(),
            b"keep me",
            "old values still open"
        );
        let resealed = rotated
            .rekey("p", "r", &sealed)
            .unwrap()
            .expect("sealed by an old key");
        assert_eq!(
            rotated.rekey("p", "r", &resealed).unwrap(),
            None,
            "already under the current key"
        );
        let only_new = vault(&format!("k2 current {}", key(2)));
        assert_eq!(
            only_new.open("p", "r", &resealed).unwrap(),
            b"keep me",
            "the old key can be removed"
        );
        assert_eq!(only_new.open("p", "r", &sealed), Err(VaultError::Open));
    }

    #[test]
    fn the_canary_proves_the_key_works() {
        let v = one_key();
        let canary = v.seal_canary().unwrap();
        assert!(v.verify_canary(&canary).is_ok());
        assert!(
            vault(&format!("k1 current {}", key(5)))
                .verify_canary(&canary)
                .is_err()
        );
        assert!(
            v.verify_canary(&v.seal("vault_canary", "canary", b"something else").unwrap())
                .is_err()
        );
    }

    #[test]
    fn nonces_come_from_the_injected_random_source() {
        let a = EnvelopeVault::from_key_text(
            &format!("k1 current {}", key(1)),
            Arc::new(FakeRng::new(1)),
        )
        .unwrap();
        let b = EnvelopeVault::from_key_text(
            &format!("k1 current {}", key(1)),
            Arc::new(FakeRng::new(1)),
        )
        .unwrap();
        assert_eq!(
            a.seal("p", "r", b"x").unwrap(),
            b.seal("p", "r", b"x").unwrap()
        );
    }

    #[test]
    fn key_files_are_read_from_disk_and_debug_hides_key_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("master.key");
        std::fs::write(&path, format!("dev-1 current {}\n", key(3))).unwrap();
        let v = EnvelopeVault::from_key_file(&path, Arc::new(SystemRng)).unwrap();
        assert_eq!(v.current_key_id(), "dev-1");
        assert!(
            !format!("{v:?}").contains("AwMD"),
            "base64 of the key must not appear"
        );
        assert_eq!(
            EnvelopeVault::from_key_file(&dir.path().join("missing"), Arc::new(SystemRng))
                .unwrap_err(),
            KeyFileError::Unreadable
        );
    }
}
