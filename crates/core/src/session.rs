//! Sign-ins (authentication sessions): scope, expiry and ending (BR-4.1, BR-8).

use crate::ids::{AuthSessionId, UserId};
use crate::token::TokenHash;
use time::{Duration, OffsetDateTime};

/// `last_seen_at` is written at most this often (NFR design, performance).
pub const TOUCH_INTERVAL: Duration = Duration::seconds(60);

/// What a sign-in may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Everything the role allows.
    Full,
    /// Only MFA enrolment, sign-out and the current-user lookup (BR-4.1).
    MfaEnrolmentOnly,
}

impl Scope {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::MfaEnrolmentOnly => "mfa_enrolment_only",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "full" => Some(Self::Full),
            "mfa_enrolment_only" => Some(Self::MfaEnrolmentOnly),
            _ => None,
        }
    }
}

/// Why a sign-in ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndReason {
    /// The user signed out.
    SignedOut,
    /// Idle for longer than the idle timeout.
    IdleTimeout,
    /// Older than the absolute duration.
    AbsoluteTimeout,
    /// The user changed their password (other sign-ins end, BR-8.3).
    PasswordChanged,
    /// The account was deactivated (BR-8.4).
    Deactivated,
    /// An admin issued a reset link (BR-6.3).
    ResetIssued,
}

impl EndReason {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignedOut => "signed_out",
            Self::IdleTimeout => "idle_timeout",
            Self::AbsoluteTimeout => "absolute_timeout",
            Self::PasswordChanged => "password_changed",
            Self::Deactivated => "deactivated",
            Self::ResetIssued => "reset_issued",
        }
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::SignedOut,
            Self::IdleTimeout,
            Self::AbsoluteTimeout,
            Self::PasswordChanged,
            Self::Deactivated,
            Self::ResetIssued,
        ]
        .into_iter()
        .find(|reason| reason.as_str() == text)
    }
}

/// Whether a sign-in may still be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Usable.
    Active,
    /// Already ended.
    Ended(EndReason),
    /// Idle for too long.
    IdleExpired,
    /// Past its absolute duration.
    AbsoluteExpired,
}

/// A sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    /// Identifier.
    pub id: AuthSessionId,
    /// The signed-in user.
    pub user_id: UserId,
    /// Hash of the cookie token (the token itself is never stored).
    pub token_hash: TokenHash,
    /// When the sign-in began.
    pub created_at: OffsetDateTime,
    /// When the sign-in was last used.
    pub last_seen_at: OffsetDateTime,
    /// Idle timeout in force when the sign-in began (BR-8.2).
    pub idle_timeout: Duration,
    /// Absolute duration in force when the sign-in began (BR-8.2).
    pub absolute_duration: Duration,
    /// What the sign-in may do.
    pub scope: Scope,
    /// When the sign-in ended, if it has.
    pub ended_at: Option<OffsetDateTime>,
    /// Why it ended.
    pub ended_reason: Option<EndReason>,
    /// Source address and client summary, for audit.
    pub source_summary: Option<String>,
}

impl AuthSession {
    /// Check the sign-in at `now`.
    pub fn status(&self, now: OffsetDateTime) -> SessionStatus {
        if let Some(reason) = self.ended_reason {
            return SessionStatus::Ended(reason);
        }
        if now - self.created_at > self.absolute_duration {
            return SessionStatus::AbsoluteExpired;
        }
        if now - self.last_seen_at > self.idle_timeout {
            return SessionStatus::IdleExpired;
        }
        SessionStatus::Active
    }

    /// Whether `last_seen_at` should be written now (at most once a minute).
    pub fn needs_touch(&self, now: OffsetDateTime) -> bool {
        now - self.last_seen_at >= TOUCH_INTERVAL
    }

    /// Record use of the sign-in.
    pub fn touch(&mut self, now: OffsetDateTime) {
        self.last_seen_at = now;
    }

    /// End the sign-in (an already-ended sign-in keeps its first reason).
    pub fn end(&mut self, reason: EndReason, now: OffsetDateTime) {
        if self.ended_reason.is_none() {
            self.ended_reason = Some(reason);
            self.ended_at = Some(now);
        }
    }

    /// Upgrade an enrolment-only sign-in to full scope once MFA is confirmed.
    pub fn upgrade_to_full(&mut self) {
        self.scope = Scope::Full;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    fn session(now: OffsetDateTime) -> AuthSession {
        let clock = FakeClock::new(now);
        let rng = FakeRng::new(1);
        AuthSession {
            id: AuthSessionId::generate(&clock, &rng).unwrap(),
            user_id: UserId::generate(&clock, &rng).unwrap(),
            token_hash: crate::token::hash_token("t"),
            created_at: now,
            last_seen_at: now,
            idle_timeout: Duration::hours(12),
            absolute_duration: Duration::days(14),
            scope: Scope::Full,
            ended_at: None,
            ended_reason: None,
            source_summary: None,
        }
    }

    #[test]
    fn br_8_2_idle_timeout_applies_from_last_use() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let mut s = session(start);
        assert_eq!(s.status(start + Duration::hours(12)), SessionStatus::Active);
        assert_eq!(
            s.status(start + Duration::hours(12) + Duration::seconds(1)),
            SessionStatus::IdleExpired
        );
        s.touch(start + Duration::hours(11));
        assert_eq!(s.status(start + Duration::hours(20)), SessionStatus::Active);
    }

    #[test]
    fn br_8_2_absolute_duration_applies_regardless_of_activity() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let mut s = session(start);
        s.touch(start + Duration::days(14));
        assert_eq!(
            s.status(start + Duration::days(14) + Duration::seconds(1)),
            SessionStatus::AbsoluteExpired
        );
    }

    #[test]
    fn br_8_2_the_limits_are_those_stored_on_the_session() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let mut s = session(start);
        s.idle_timeout = Duration::minutes(15);
        assert_eq!(
            s.status(start + Duration::minutes(16)),
            SessionStatus::IdleExpired
        );
    }

    #[test]
    fn br_8_3_an_ended_session_stays_ended_with_its_first_reason() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let mut s = session(start);
        s.end(EndReason::PasswordChanged, start);
        s.end(EndReason::SignedOut, start + Duration::seconds(5));
        assert_eq!(
            s.status(start),
            SessionStatus::Ended(EndReason::PasswordChanged)
        );
        assert_eq!(s.ended_at, Some(start));
    }

    #[test]
    fn last_seen_is_written_at_most_once_a_minute() {
        let start = OffsetDateTime::UNIX_EPOCH;
        let s = session(start);
        assert!(!s.needs_touch(start + Duration::seconds(59)));
        assert!(s.needs_touch(start + Duration::seconds(60)));
    }

    #[test]
    fn br_4_1_enrolment_only_sessions_upgrade_to_full() {
        let mut s = session(OffsetDateTime::UNIX_EPOCH);
        s.scope = Scope::MfaEnrolmentOnly;
        s.upgrade_to_full();
        assert_eq!(s.scope, Scope::Full);
    }

    #[test]
    fn text_forms_round_trip() {
        for reason in [
            EndReason::SignedOut,
            EndReason::IdleTimeout,
            EndReason::AbsoluteTimeout,
            EndReason::PasswordChanged,
            EndReason::Deactivated,
            EndReason::ResetIssued,
        ] {
            assert_eq!(EndReason::parse(reason.as_str()), Some(reason));
        }
        for scope in [Scope::Full, Scope::MfaEnrolmentOnly] {
            assert_eq!(Scope::parse(scope.as_str()), Some(scope));
        }
    }
}
