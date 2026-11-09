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
mod tests {
    use super::*;
    use tandem_support::SystemRng;

    /// RFC 6238 appendix B, SHA-1 secret "12345678901234567890". The RFC lists 8-digit values; the
    /// 6-digit code is the last six digits of the same truncated value.
    const SECRET: &[u8] = b"12345678901234567890";

    fn totp() -> HmacTotp {
        HmacTotp::new(Arc::new(SystemRng))
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).unwrap()
    }

    #[test]
    fn br_4_3_matches_the_rfc_6238_test_vectors() {
        for (time, code) in [
            (59_i64, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
            (20_000_000_000, "353130"),
        ] {
            let step = u64::try_from(time).unwrap() / 30;
            assert_eq!(
                HmacTotp::code_for_step(SECRET, step).as_deref(),
                Some(code),
                "t={time}"
            );
        }
    }

    #[test]
    fn br_4_3_a_code_is_accepted_one_step_either_side_but_not_two() {
        let service = totp();
        let t = 1_234_567_890_i64;
        let code = HmacTotp::code_for_step(SECRET, u64::try_from(t).unwrap() / 30).unwrap();
        assert_eq!(
            service.verify(SECRET, &code, at(t)),
            Some(u64::try_from(t).unwrap() / 30)
        );
        assert!(
            service.verify(SECRET, &code, at(t + 30)).is_some(),
            "one step later"
        );
        assert!(
            service.verify(SECRET, &code, at(t - 30)).is_some(),
            "one step earlier"
        );
        assert!(
            service.verify(SECRET, &code, at(t + 90)).is_none(),
            "three steps later"
        );
        assert!(service.verify(SECRET, &code, at(t - 90)).is_none());
    }

    #[test]
    fn br_4_4_the_matching_step_is_reported_so_replays_can_be_refused() {
        let service = totp();
        let t = 1_800_000_000_i64;
        let step = u64::try_from(t).unwrap() / 30;
        let previous = HmacTotp::code_for_step(SECRET, step - 1).unwrap();
        assert_eq!(service.verify(SECRET, &previous, at(t)), Some(step - 1));
    }

    #[test]
    fn malformed_codes_are_rejected_without_computing_anything() {
        let service = totp();
        for bad in ["", "12345", "1234567", "12345a", " 12345", "١٢٣٤٥٦"] {
            assert!(service.verify(SECRET, bad, at(59)).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn secrets_are_20_random_bytes_and_base32_has_no_padding() {
        let service = totp();
        let a = service.generate_secret().unwrap();
        assert_eq!(a.len(), 20);
        assert_ne!(a, service.generate_secret().unwrap());
        let text = service.secret_base32(SECRET);
        assert_eq!(text, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert!(!text.contains('='));
    }

    #[test]
    fn the_otpauth_uri_escapes_the_label_and_carries_the_parameters() {
        let uri = totp().otpauth_uri(SECRET, "ana+test@example.com", "Tandem Co");
        assert!(uri.starts_with(
            "otpauth://totp/Tandem%20Co:ana%2Btest%40example.com?secret=GEZDGNBVGY3TQOJQ"
        ));
        assert!(uri.contains("&issuer=Tandem%20Co&algorithm=SHA1&digits=6&period=30"));
    }
}
