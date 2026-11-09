//! Multi-factor authentication credentials and recovery codes (BR-4).

use crate::ids::{RecoveryCodeId, UserId};
use crate::token::TokenHash;
use time::OffsetDateTime;

/// Number of recovery codes in a set (BR-4.5).
pub const RECOVERY_CODE_COUNT: usize = 10;
/// A reminder to regenerate appears when this many or fewer unused codes remain (BR-4 workflow W6).
pub const RECOVERY_REMINDER_THRESHOLD: usize = 3;

/// Why an MFA operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MfaError {
    /// The code's time-step was already accepted (BR-4.4).
    #[error("this code was already used")]
    Replay,
    /// The credential is already confirmed.
    #[error("MFA is already enrolled")]
    AlreadyConfirmed,
    /// The recovery code was already used.
    #[error("this recovery code was already used")]
    RecoveryCodeUsed,
}

/// A user's TOTP credential. At most one per user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MfaCredential {
    /// The owner.
    pub user_id: UserId,
    /// The TOTP secret, sealed by the vault.
    pub secret_sealed: Vec<u8>,
    /// Empty until the user proves the first valid code.
    pub confirmed_at: Option<OffsetDateTime>,
    /// The latest accepted time-step, to refuse reuse.
    pub last_used_step: Option<u64>,
}

impl MfaCredential {
    /// A new, unconfirmed credential.
    pub fn new(user_id: UserId, secret_sealed: Vec<u8>) -> Self {
        Self {
            user_id,
            secret_sealed,
            confirmed_at: None,
            last_used_step: None,
        }
    }

    /// Whether enrolment is complete.
    pub fn is_confirmed(&self) -> bool {
        self.confirmed_at.is_some()
    }

    /// Accept a code's time-step: it must be later than any already accepted (BR-4.4).
    ///
    /// # Errors
    /// [`MfaError::Replay`] if the step was already used.
    pub fn accept_step(&mut self, step: u64) -> Result<(), MfaError> {
        if self.last_used_step.is_some_and(|last| step <= last) {
            return Err(MfaError::Replay);
        }
        self.last_used_step = Some(step);
        Ok(())
    }

    /// Mark enrolment complete.
    ///
    /// # Errors
    /// [`MfaError::AlreadyConfirmed`] if it already is.
    pub fn confirm(&mut self, now: OffsetDateTime) -> Result<(), MfaError> {
        if self.is_confirmed() {
            return Err(MfaError::AlreadyConfirmed);
        }
        self.confirmed_at = Some(now);
        Ok(())
    }
}

/// One single-use recovery code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCode {
    /// Identifier.
    pub id: RecoveryCodeId,
    /// The owner.
    pub user_id: UserId,
    /// Hash of the normalised code.
    pub code_hash: TokenHash,
    /// Set when used.
    pub used_at: Option<OffsetDateTime>,
}

impl RecoveryCode {
    /// Consume the code.
    ///
    /// # Errors
    /// [`MfaError::RecoveryCodeUsed`] if it was already used.
    pub fn use_code(&mut self, now: OffsetDateTime) -> Result<(), MfaError> {
        if self.used_at.is_some() {
            return Err(MfaError::RecoveryCodeUsed);
        }
        self.used_at = Some(now);
        Ok(())
    }
}

/// Whether the user should be reminded to regenerate their recovery codes.
pub fn should_remind_to_regenerate(unused_codes: usize) -> bool {
    unused_codes <= RECOVERY_REMINDER_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    fn user() -> UserId {
        UserId::generate(&FakeClock::at_unix(0), &FakeRng::new(1)).unwrap()
    }

    #[test]
    fn br_4_4_a_time_step_cannot_be_reused_or_go_backwards() {
        let mut credential = MfaCredential::new(user(), vec![1]);
        credential.accept_step(100).unwrap();
        assert_eq!(credential.accept_step(100), Err(MfaError::Replay));
        assert_eq!(credential.accept_step(99), Err(MfaError::Replay));
        credential.accept_step(101).unwrap();
    }

    #[test]
    fn br_4_1_enrolment_completes_once() {
        let mut credential = MfaCredential::new(user(), vec![1]);
        assert!(!credential.is_confirmed());
        credential.confirm(OffsetDateTime::UNIX_EPOCH).unwrap();
        assert!(credential.is_confirmed());
        assert_eq!(
            credential.confirm(OffsetDateTime::UNIX_EPOCH),
            Err(MfaError::AlreadyConfirmed)
        );
    }

    #[test]
    fn br_4_5_a_recovery_code_is_single_use() {
        let clock = FakeClock::at_unix(0);
        let rng = FakeRng::new(1);
        let mut code = RecoveryCode {
            id: RecoveryCodeId::generate(&clock, &rng).unwrap(),
            user_id: user(),
            code_hash: crate::token::hash_recovery_code("abcd-efgh-jkmn"),
            used_at: None,
        };
        code.use_code(OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            code.use_code(OffsetDateTime::UNIX_EPOCH),
            Err(MfaError::RecoveryCodeUsed)
        );
    }

    #[test]
    fn br_4_5_a_set_has_ten_codes_and_a_reminder_shows_at_three_or_fewer() {
        assert_eq!(RECOVERY_CODE_COUNT, 10);
        assert!(!should_remind_to_regenerate(4));
        assert!(should_remind_to_regenerate(3));
        assert!(should_remind_to_regenerate(0));
    }
}
