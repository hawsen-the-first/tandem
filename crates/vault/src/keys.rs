//! The key file: one key per line, `<key-id> <current|decrypt-only> <base64 of 32 bytes>`.
//! Lines starting with `#` and blank lines are ignored. Error messages never include key material.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use zeroize::Zeroizing;

/// Whether a key may seal new values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStatus {
    /// Seals new values. Exactly one key has this status.
    Current,
    /// Only opens values sealed earlier.
    DecryptOnly,
}

/// Why a key file was rejected.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyFileError {
    /// The file could not be read.
    #[error("the key file could not be read")]
    Unreadable,
    /// A line does not have three fields.
    #[error("line {0}: expected `<key-id> <current|decrypt-only> <base64 key>`")]
    BadLine(usize),
    /// A key id is empty, too long or has unsupported characters.
    #[error("line {0}: the key id must be 1 to 64 characters from A-Z a-z 0-9 . _ -")]
    BadKeyId(usize),
    /// The status is neither `current` nor `decrypt-only`.
    #[error("line {0}: the status must be `current` or `decrypt-only`")]
    BadStatus(usize),
    /// The key is not valid base64 of exactly 32 bytes.
    #[error("line {0}: the key must be base64 of exactly 32 bytes")]
    BadKey(usize),
    /// Two keys share an id.
    #[error("line {0}: duplicate key id")]
    DuplicateKeyId(usize),
    /// There is no key at all.
    #[error("the key file contains no keys")]
    NoKeys,
    /// There is not exactly one current key.
    #[error("exactly one key must be `current`")]
    CurrentKeyCount,
}

/// One parsed key.
pub(crate) struct Key {
    pub(crate) id: String,
    pub(crate) status: KeyStatus,
    pub(crate) bytes: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key")
            .field("id", &self.id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

fn valid_key_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Parse the key file text.
pub(crate) fn parse(text: &str) -> Result<Vec<Key>, KeyFileError> {
    let mut keys: Vec<Key> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [id, status, encoded] = fields[..] else {
            return Err(KeyFileError::BadLine(number));
        };
        if !valid_key_id(id) {
            return Err(KeyFileError::BadKeyId(number));
        }
        let status = match status {
            "current" => KeyStatus::Current,
            "decrypt-only" => KeyStatus::DecryptOnly,
            _ => return Err(KeyFileError::BadStatus(number)),
        };
        let decoded = Zeroizing::new(
            STANDARD
                .decode(encoded)
                .map_err(|_| KeyFileError::BadKey(number))?,
        );
        let bytes: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| KeyFileError::BadKey(number))?;
        if keys.iter().any(|key| key.id == id) {
            return Err(KeyFileError::DuplicateKeyId(number));
        }
        keys.push(Key {
            id: id.to_owned(),
            status,
            bytes: Zeroizing::new(bytes),
        });
    }
    if keys.is_empty() {
        return Err(KeyFileError::NoKeys);
    }
    if keys
        .iter()
        .filter(|key| key.status == KeyStatus::Current)
        .count()
        != 1
    {
        return Err(KeyFileError::CurrentKeyCount);
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> String {
        STANDARD.encode([byte; 32])
    }

    #[test]
    fn parses_comments_blank_lines_and_several_keys() {
        let text = format!(
            "# keys\n\nk1 decrypt-only {}\nk2   current   {}\n",
            key(1),
            key(2)
        );
        let keys = parse(&text).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].status, KeyStatus::Current);
    }

    #[test]
    fn rejects_malformed_files_with_line_numbers_and_no_key_material() {
        assert_eq!(parse("").unwrap_err(), KeyFileError::NoKeys);
        assert_eq!(parse("k1 current").unwrap_err(), KeyFileError::BadLine(1));
        assert_eq!(
            parse(&format!("bad/id current {}", key(1))).unwrap_err(),
            KeyFileError::BadKeyId(1)
        );
        assert_eq!(
            parse(&format!("k1 active {}", key(1))).unwrap_err(),
            KeyFileError::BadStatus(1)
        );
        assert_eq!(
            parse("k1 current !!!notbase64").unwrap_err(),
            KeyFileError::BadKey(1)
        );
        assert_eq!(
            parse(&format!("k1 current {}", STANDARD.encode([0_u8; 16]))).unwrap_err(),
            KeyFileError::BadKey(1)
        );
        let duplicate = format!("k1 current {}\nk1 decrypt-only {}", key(1), key(2));
        assert_eq!(
            parse(&duplicate).unwrap_err(),
            KeyFileError::DuplicateKeyId(2)
        );
    }

    #[test]
    fn there_must_be_exactly_one_current_key() {
        assert_eq!(
            parse(&format!("k1 decrypt-only {}", key(1))).unwrap_err(),
            KeyFileError::CurrentKeyCount
        );
        let two = format!("k1 current {}\nk2 current {}", key(1), key(2));
        assert_eq!(parse(&two).unwrap_err(), KeyFileError::CurrentKeyCount);
    }

    #[test]
    fn debug_output_never_contains_key_bytes() {
        let keys = parse(&format!("k1 current {}", key(7))).unwrap();
        let shown = format!("{keys:?}");
        assert!(shown.contains("k1") && !shown.contains('7'));
    }
}
