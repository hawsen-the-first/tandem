//! Credential handling: the Argon2id hasher pool, the breached-password checker and TOTP.

mod breach;
mod hasher;
mod profile;
mod totp;

pub use breach::{
    BloomFilter, BloomFilterBuilder, BloomFilterError, BreachCheckerImpl, FetchError,
    HttpRangeFetcher, OnlineBreachCheck, RangeFetcher, sha1_hex_upper,
};
pub use hasher::{ArgonHasher, HasherStats, concurrency_limit};
pub use profile::{HashProfile, ProfileError, benchmark_profile};
pub use totp::HmacTotp;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-credentials";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
