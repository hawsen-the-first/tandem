//! TOTP (RFC 6238): HMAC-SHA1, 6 digits, 30-second steps, one step of tolerance (BR-4.3).

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use sha1::Sha1;
use std::sync::Arc;
use tandem_core::token::constant_time_eq;
use tandem_ports::TotpService;
use tandem_support::{Rng, RngError, random_array};
use time::OffsetDateTime;

const DIGITS: u32 = 6;
const STEP_SECONDS: u64 = 30;
const SECRET_BYTES: usize = 20;
/// Characters left unescaped in the `otpauth` label and parameters.
const LABEL: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// The production TOTP implementation.
pub struct HmacTotp {
    rng: Arc<dyn Rng>,
}

impl std::fmt::Debug for HmacTotp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HmacTotp").finish_non_exhaustive()
    }
}

impl HmacTotp {
    /// A TOTP service drawing new secrets from the given random source.
    pub fn new(rng: Arc<dyn Rng>) -> Self {
        Self { rng }
    }

    /// The code for a time-step (the HOTP value of the step counter).
    fn code_for_step(secret: &[u8], step: u64) -> Option<String> {
        let mut mac = Hmac::<Sha1>::new_from_slice(secret).ok()?;
        mac.update(&step.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = usize::from(digest[digest.len() - 1] & 0x0F);
        let truncated = u32::from_be_bytes([
            digest[offset] & 0x7F,
            digest[offset + 1],
            digest[offset + 2],
            digest[offset + 3],
        ]);
        Some(format!(
            "{:0width$}",
            truncated % 10_u32.pow(DIGITS),
            width = DIGITS as usize
        ))
    }

    fn step_at(at: OffsetDateTime) -> u64 {
        u64::try_from(at.unix_timestamp()).unwrap_or(0) / STEP_SECONDS
    }
}

impl TotpService for HmacTotp {
    fn generate_secret(&self) -> Result<Vec<u8>, RngError> {
        Ok(random_array::<SECRET_BYTES>(&*self.rng)?.to_vec())
    }

    fn secret_base32(&self, secret: &[u8]) -> String {
        BASE32_NOPAD.encode(secret)
    }

    fn otpauth_uri(&self, secret: &[u8], account: &str, issuer: &str) -> String {
        let issuer = utf8_percent_encode(issuer, LABEL);
        let account = utf8_percent_encode(account, LABEL);
        format!(
            "otpauth://totp/{issuer}:{account}?secret={}&issuer={issuer}&algorithm=SHA1&digits={DIGITS}&period={STEP_SECONDS}",
            self.secret_base32(secret)
        )
    }

    fn verify(&self, secret: &[u8], code: &str, at: OffsetDateTime) -> Option<u64> {
        if code.len() != DIGITS as usize || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let now = Self::step_at(at);
        // Check every step in the window without stopping early, so timing does not reveal which
        // step matched.
        let mut matched = None;
        for step in now.saturating_sub(1)..=now.saturating_add(1) {
            if let Some(expected) = Self::code_for_step(secret, step)
                && constant_time_eq(expected.as_bytes(), code.as_bytes())
            {
                matched = Some(step);
            }
        }
        matched
    }
}

#[cfg(test)]
mod tests;
