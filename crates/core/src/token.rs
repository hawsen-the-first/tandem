//! Tokens and codes: generation, hashing and comparison (BR-5.3, BR-8.1, BR-4.5).
//!
//! Tokens are random and high-entropy, so a fast hash is enough; only the hash is stored.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use tandem_support::{Rng, RngError, random_array};

/// The stored form of a token or code: the base64url SHA-256 of its text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenHash(String);

impl TokenHash {
    /// Wrap a value read from storage.
    pub fn from_stored(value: String) -> Self {
        Self(value)
    }

    /// The hash as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Hash a token's text.
pub fn hash_token(token: &str) -> TokenHash {
    TokenHash(URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes())))
}

/// Generate a 256-bit random token. Returns the plaintext (shown once) and its hash.
///
/// # Errors
/// [`RngError`] if the random source fails.
pub fn generate_token(rng: &dyn Rng) -> Result<(String, TokenHash), RngError> {
    let bytes = random_array::<32>(rng)?;
    let plaintext = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&plaintext);
    Ok((plaintext, hash))
}

/// Characters for recovery codes: no look-alikes (no i, l, o, 0, 1). 31 symbols.
const RECOVERY_ALPHABET: &[u8; 31] = b"abcdefghjkmnpqrstuvwxyz23456789";
const RECOVERY_CODE_CHARS: usize = 12;

/// Generate a recovery code such as `k3m9-x2pq-7tnd` (12 symbols, about 59 bits).
///
/// # Errors
/// [`RngError`] if the random source fails.
pub fn generate_recovery_code(rng: &dyn Rng) -> Result<String, RngError> {
    let mut chars = String::with_capacity(RECOVERY_CODE_CHARS + 2);
    let mut produced = 0;
    while produced < RECOVERY_CODE_CHARS {
        for byte in random_array::<32>(rng)? {
            // Rejection sampling keeps the choice uniform: 248 = 31 * 8.
            if byte < 248 && produced < RECOVERY_CODE_CHARS {
                if produced > 0 && produced % 4 == 0 {
                    chars.push('-');
                }
                chars.push(char::from(RECOVERY_ALPHABET[usize::from(byte) % 31]));
                produced += 1;
            }
        }
    }
    Ok(chars)
}

/// Normalise a typed recovery code: drop separators and spaces, lower-case.
pub fn normalize_recovery_code(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The stored hash of a recovery code (after normalisation).
pub fn hash_recovery_code(raw: &str) -> TokenHash {
    hash_token(&format!("recovery:{}", normalize_recovery_code(raw)))
}

/// Compare two byte strings in constant time (for CSRF tokens and similar).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeRng, SystemRng};

    #[test]
    fn br_8_1_tokens_are_256_bit_and_only_the_hash_is_derivable_from_them() {
        let (plaintext, hash) = generate_token(&SystemRng).unwrap();
        assert_eq!(URL_SAFE_NO_PAD.decode(&plaintext).unwrap().len(), 32);
        assert_eq!(hash, hash_token(&plaintext));
        assert_ne!(hash.as_str(), plaintext);
        let (other, _) = generate_token(&SystemRng).unwrap();
        assert_ne!(plaintext, other);
    }

    #[test]
    fn br_4_5_recovery_codes_use_the_safe_alphabet_and_format() {
        let code = generate_recovery_code(&FakeRng::new(3)).unwrap();
        assert_eq!(code.len(), 14);
        for (i, c) in code.chars().enumerate() {
            if i == 4 || i == 9 {
                assert_eq!(c, '-');
            } else {
                assert!(
                    RECOVERY_ALPHABET.contains(&(c as u8)),
                    "{c} not in alphabet"
                );
            }
        }
    }

    #[test]
    fn br_4_5_recovery_codes_are_unique_across_a_set_of_ten() {
        let rng = SystemRng;
        let mut codes: Vec<String> = (0..10)
            .map(|_| generate_recovery_code(&rng).unwrap())
            .collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), 10);
    }

    #[test]
    fn br_4_5_recovery_code_matching_ignores_case_dashes_and_spaces() {
        assert_eq!(
            hash_recovery_code("K3M9-X2PQ-7TND"),
            hash_recovery_code(" k3m9x2pq7tnd ")
        );
        assert_ne!(
            hash_recovery_code("k3m9-x2pq-7tnd"),
            hash_recovery_code("k3m9-x2pq-7tne")
        );
    }

    #[test]
    fn constant_time_eq_compares_content_and_length() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
