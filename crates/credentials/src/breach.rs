//! The breached-password check (BR-2.3): an offline Bloom filter that is always consulted, and an
//! optional online k-anonymity range query guarded by a circuit breaker (RL-2).

use async_trait::async_trait;
use sha1::{Digest, Sha1};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tandem_ports::BreachChecker;
use tandem_support::{CallError, CircuitBreaker};

const MAGIC: &[u8; 8] = b"TNDMBF01";
const HEADER_LEN: usize = 8 + 4 + 8;
const MAX_HASHES: u32 = 30;
/// The embedded filter built by `tools/breach-filter` (see the data and licence notes in the docs).
const EMBEDDED_FILTER: &[u8] = include_bytes!("../data/breached-passwords.bloom");

/// Why a filter file was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BloomFilterError {
    /// Not a Tandem filter, or truncated.
    #[error("the breached-password filter file is not valid")]
    Invalid,
    /// A hash line was not 40 hexadecimal characters.
    #[error("a password hash line is not 40 hexadecimal characters")]
    BadHashLine,
}

/// SHA-1 of the password, upper-case hexadecimal (the form used by the breach corpus).
pub fn sha1_hex_upper(password: &str) -> String {
    data_encoding::HEXUPPER.encode(&Sha1::digest(password.as_bytes()))
}

fn digest_of(password: &str) -> [u8; 20] {
    Sha1::digest(password.as_bytes()).into()
}

/// A Bloom filter over password SHA-1 digests. It can say "possibly breached" (false positives
/// are rare and configured at build time) but never wrongly says "not breached" for a listed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloomFilter {
    hashes: u32,
    bit_count: u64,
    bits: Vec<u8>,
}

fn bit_positions(digest: &[u8; 20], hashes: u32, bit_count: u64) -> impl Iterator<Item = u64> {
    let first = u64::from_le_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    let step = u64::from_le_bytes([
        digest[8], digest[9], digest[10], digest[11], digest[12], digest[13], digest[14],
        digest[15],
    ]) | 1;
    (0..u64::from(hashes)).map(move |i| first.wrapping_add(i.wrapping_mul(step)) % bit_count)
}

impl BloomFilter {
    /// The filter compiled into this binary.
    ///
    /// # Errors
    /// [`BloomFilterError::Invalid`] if the embedded data is corrupt (a build problem).
    pub fn embedded() -> Result<Self, BloomFilterError> {
        Self::from_bytes(EMBEDDED_FILTER)
    }

    /// Parse a filter file.
    ///
    /// # Errors
    /// [`BloomFilterError::Invalid`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BloomFilterError> {
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            return Err(BloomFilterError::Invalid);
        }
        let hashes = u32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| BloomFilterError::Invalid)?,
        );
        let bit_count = u64::from_le_bytes(
            bytes[12..20]
                .try_into()
                .map_err(|_| BloomFilterError::Invalid)?,
        );
        let expected_len =
            usize::try_from(bit_count.div_ceil(8)).map_err(|_| BloomFilterError::Invalid)?;
        let bits = bytes[HEADER_LEN..].to_vec();
        if hashes == 0 || hashes > MAX_HASHES || bit_count == 0 || bits.len() != expected_len {
            return Err(BloomFilterError::Invalid);
        }
        Ok(Self {
            hashes,
            bit_count,
            bits,
        })
    }

    /// The file form.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.bits.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.hashes.to_le_bytes());
        out.extend_from_slice(&self.bit_count.to_le_bytes());
        out.extend_from_slice(&self.bits);
        out
    }

    fn contains_digest(&self, digest: &[u8; 20]) -> bool {
        bit_positions(digest, self.hashes, self.bit_count).all(|position| {
            let byte = usize::try_from(position / 8).unwrap_or(usize::MAX);
            self.bits
                .get(byte)
                .is_some_and(|b| b & (1 << (position % 8)) != 0)
        })
    }

    /// Whether the password is (probably) in the list.
    pub fn contains_password(&self, password: &str) -> bool {
        self.contains_digest(&digest_of(password))
    }

    /// Size of the file form in bytes.
    pub fn byte_len(&self) -> usize {
        HEADER_LEN + self.bits.len()
    }
}

/// Builds a [`BloomFilter`] sized for an expected number of entries and false-positive rate.
#[derive(Debug)]
pub struct BloomFilterBuilder {
    filter: BloomFilter,
    inserted: u64,
}

impl BloomFilterBuilder {
    /// Size a filter for `expected_items` entries at `false_positive_rate` (for example `1e-5`).
    pub fn with_capacity(expected_items: u64, false_positive_rate: f64) -> Self {
        let n = expected_items.max(1) as f64;
        let p = false_positive_rate.clamp(1e-12, 0.5);
        let ln2 = std::f64::consts::LN_2;
        let bit_count = ((-n * p.ln()) / (ln2 * ln2)).ceil().max(64.0) as u64;
        let hashes = (((bit_count as f64 / n) * ln2).round() as u32).clamp(1, MAX_HASHES);
        let bytes = usize::try_from(bit_count.div_ceil(8)).unwrap_or(usize::MAX);
        Self {
            filter: BloomFilter {
                hashes,
                bit_count,
                bits: vec![0; bytes],
            },
            inserted: 0,
        }
    }

    fn set(&mut self, digest: &[u8; 20]) {
        for position in bit_positions(digest, self.filter.hashes, self.filter.bit_count) {
            let byte = usize::try_from(position / 8).unwrap_or(usize::MAX);
            if let Some(slot) = self.filter.bits.get_mut(byte) {
                *slot |= 1 << (position % 8);
            }
        }
        self.inserted += 1;
    }

    /// Add a password in plain text.
    pub fn insert_password(&mut self, password: &str) {
        self.set(&digest_of(password));
    }

    /// Add a line of the breach corpus: 40 hexadecimal characters of SHA-1, optionally followed by
    /// `:count`.
    ///
    /// # Errors
    /// [`BloomFilterError::BadHashLine`].
    pub fn insert_hash_line(&mut self, line: &str) -> Result<(), BloomFilterError> {
        let hex = line.trim().split(':').next().unwrap_or_default();
        let bytes = data_encoding::HEXUPPER_PERMISSIVE
            .decode(hex.as_bytes())
            .map_err(|_| BloomFilterError::BadHashLine)?;
        let digest: [u8; 20] = bytes
            .try_into()
            .map_err(|_| BloomFilterError::BadHashLine)?;
        self.set(&digest);
        Ok(())
    }

    /// How many entries were added.
    pub fn inserted(&self) -> u64 {
        self.inserted
    }

    /// Finish.
    pub fn finish(self) -> BloomFilter {
        self.filter
    }
}

/// Why an online range fetch failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the breach range request failed: {0}")]
pub struct FetchError(pub String);

/// Fetches the k-anonymity range for a 5-character SHA-1 prefix. Only the prefix leaves the host.
#[async_trait]
pub trait RangeFetcher: Send + Sync {
    /// The body of the range response: lines of `SUFFIX:COUNT`.
    async fn fetch_range(&self, prefix: &str) -> Result<String, FetchError>;
}

/// The real fetcher: the Have I Been Pwned range API over HTTPS.
#[derive(Debug, Clone)]
pub struct HttpRangeFetcher {
    client: reqwest::Client,
    base_url: String,
}

impl HttpRangeFetcher {
    /// Create a fetcher with the given request timeout.
    ///
    /// # Errors
    /// [`FetchError`] when the HTTP client cannot be built.
    #[allow(
        clippy::disallowed_methods,
        reason = "fixed public host (api.pwnedpasswords.com), no administrator-supplied URL"
    )]
    pub fn new(timeout: Duration) -> Result<Self, FetchError> {
        // reqwest is built without a bundled crypto provider; use `ring`, as the database driver
        // does. Installing twice is harmless.
        let _already_installed = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("tandem-breach-check")
            .build()
            .map_err(|error| FetchError(error.to_string()))?;
        Ok(Self {
            client,
            base_url: "https://api.pwnedpasswords.com/range".to_owned(),
        })
    }
}

#[async_trait]
impl RangeFetcher for HttpRangeFetcher {
    async fn fetch_range(&self, prefix: &str) -> Result<String, FetchError> {
        let response = self
            .client
            .get(format!("{}/{prefix}", self.base_url))
            .header("Add-Padding", "true")
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| FetchError(error.to_string()))?;
        response
            .text()
            .await
            .map_err(|error| FetchError(error.to_string()))
    }
}

/// The optional online check: a fetcher behind a circuit breaker.
pub struct OnlineBreachCheck {
    fetcher: Arc<dyn RangeFetcher>,
    breaker: CircuitBreaker,
    timeout: Duration,
}

impl OnlineBreachCheck {
    /// Combine a fetcher with its breaker and a hard timeout (2 s by default, RL-2).
    pub fn new(fetcher: Arc<dyn RangeFetcher>, breaker: CircuitBreaker, timeout: Duration) -> Self {
        Self {
            fetcher,
            breaker,
            timeout,
        }
    }
}

impl std::fmt::Debug for OnlineBreachCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnlineBreachCheck").finish_non_exhaustive()
    }
}

fn body_lists_suffix(body: &str, suffix: &str) -> bool {
    body.lines().any(|line| {
        let mut parts = line.trim().splitn(2, ':');
        let found = parts
            .next()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(suffix));
        // Padding entries have a count of zero and never mean "breached".
        let count = parts
            .next()
            .and_then(|count| count.trim().parse::<u64>().ok())
            .unwrap_or(1);
        found && count > 0
    })
}

/// The breach checker used by the application.
pub struct BreachCheckerImpl {
    filter: Arc<BloomFilter>,
    online: Option<OnlineBreachCheck>,
    degraded_total: AtomicU64,
}

impl std::fmt::Debug for BreachCheckerImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BreachCheckerImpl")
            .field("online", &self.online.is_some())
            .finish_non_exhaustive()
    }
}

impl BreachCheckerImpl {
    /// A checker over the offline filter only.
    pub fn offline(filter: Arc<BloomFilter>) -> Self {
        Self {
            filter,
            online: None,
            degraded_total: AtomicU64::new(0),
        }
    }

    /// A checker that can also ask the online service when `use_online` is set.
    pub fn with_online(filter: Arc<BloomFilter>, online: OnlineBreachCheck) -> Self {
        Self {
            filter,
            online: Some(online),
            degraded_total: AtomicU64::new(0),
        }
    }

    /// How often the online check was skipped or failed (`breach_check_degraded_total`).
    pub fn degraded_total(&self) -> u64 {
        self.degraded_total.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl BreachChecker for BreachCheckerImpl {
    async fn is_breached(&self, password: &str, use_online: bool) -> bool {
        if self.filter.contains_password(password) {
            return true;
        }
        let Some(online) = self.online.as_ref().filter(|_| use_online) else {
            return false;
        };
        let hash = sha1_hex_upper(password);
        let (prefix, suffix) = hash.split_at(5);
        let fetch = async {
            tokio::time::timeout(online.timeout, online.fetcher.fetch_range(prefix))
                .await
                .unwrap_or_else(|_| Err(FetchError("timed out".to_owned())))
        };
        match online.breaker.call(fetch).await {
            Ok(body) => body_lists_suffix(&body, suffix),
            Err(failure) => {
                // The offline result stands; the online service is optional (BR-2.3, RL-2).
                self.degraded_total.fetch_add(1, Ordering::Relaxed);
                let reason = match failure {
                    CallError::Open => "circuit breaker open".to_owned(),
                    CallError::Inner(error) => error.to_string(),
                };
                tracing::warn!(%reason, "online breached-password check unavailable; using the offline list only");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
