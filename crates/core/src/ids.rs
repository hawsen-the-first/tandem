//! Typed identifiers (UUIDv7: time-ordered, generated from the injected clock and random source).

use std::fmt;
use tandem_support::{Clock, Rng, RngError, random_array};
use uuid::Uuid;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new time-ordered identifier.
            ///
            /// # Errors
            /// Returns [`RngError`] if the random source fails.
            pub fn generate(clock: &dyn Clock, rng: &dyn Rng) -> Result<Self, RngError> {
                let random = random_array::<10>(rng)?;
                let millis = u64::try_from(clock.now().unix_timestamp_nanos() / 1_000_000)
                    .unwrap_or(0);
                Ok(Self(uuid::Builder::from_unix_timestamp_millis(millis, &random).into_uuid()))
            }

            /// Wrap an existing UUID (for example one read from storage).
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.hyphenated())
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    };
}

id_type!(/// Identifies a user.
    UserId);
id_type!(/// Identifies an invitation.
    InvitationId);
id_type!(/// Identifies a password reset link.
    PasswordResetId);
id_type!(/// Identifies a sign-in (authentication session).
    AuthSessionId);
id_type!(/// Identifies a recovery code.
    RecoveryCodeId);
id_type!(/// Identifies an audit entry.
    AuditEntryId);
id_type!(/// Identifies a skeleton chat session.
    ChatSessionId);
id_type!(/// Identifies a skeleton chat message.
    ChatMessageId);
id_type!(/// Identifies a job.
    JobId);
id_type!(/// Identifies a model provider.
    ProviderId);
id_type!(/// Identifies a model listed under a provider.
    ProviderModelId);
id_type!(/// Identifies a configuration test run.
    TestRunId);

/// Identifies an outbox event; assigned by the store as an increasing sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutboxEventId(pub i64);

impl fmt::Display for OutboxEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    #[test]
    fn ids_are_time_ordered_unique_and_round_trip() {
        let clock = FakeClock::at_unix(1_700_000_000);
        let rng = FakeRng::new(1);
        let first = UserId::generate(&clock, &rng).unwrap();
        clock.advance(time::Duration::seconds(1));
        let second = UserId::generate(&clock, &rng).unwrap();
        assert!(first < second);
        assert_ne!(first, second);
        assert_eq!(first.to_string().parse::<UserId>().unwrap(), first);
        assert_eq!(first.as_uuid().get_version_num(), 7);
    }
}
