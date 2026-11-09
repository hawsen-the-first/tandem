//! Instance security settings and their ranges (BR-10, FR-1.9).

use std::collections::BTreeMap;
use time::Duration;

/// Smallest sign-in idle timeout.
pub const IDLE_MIN: Duration = Duration::minutes(15);
/// Largest sign-in idle timeout.
pub const IDLE_MAX: Duration = Duration::days(30);
/// Smallest absolute sign-in duration.
pub const ABSOLUTE_MIN: Duration = Duration::hours(1);
/// Largest absolute sign-in duration.
pub const ABSOLUTE_MAX: Duration = Duration::days(90);
/// Default idle timeout.
pub const IDLE_DEFAULT: Duration = Duration::hours(12);
/// Default absolute duration.
pub const ABSOLUTE_DEFAULT: Duration = Duration::days(14);

/// Stored key of the idle timeout, in seconds.
pub const KEY_IDLE_TIMEOUT: &str = "signin.idle_timeout";
/// Stored key of the absolute duration, in seconds.
pub const KEY_ABSOLUTE_DURATION: &str = "signin.absolute_duration";
/// Stored key of the online breach-check switch (`true` or `false`).
pub const KEY_ONLINE_BREACH_CHECK: &str = "password.online_breach_check";

/// Why settings were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    /// Idle timeout outside 15 minutes to 30 days.
    #[error("the idle timeout must be between 15 minutes and 30 days")]
    IdleOutOfRange,
    /// Absolute duration outside 1 hour to 90 days.
    #[error("the total sign-in duration must be between 1 hour and 90 days")]
    AbsoluteOutOfRange,
    /// Idle timeout longer than the absolute duration.
    #[error("the idle timeout cannot be longer than the total sign-in duration")]
    IdleExceedsAbsolute,
}

/// The settings an admin can edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecuritySettings {
    /// Sign-out after this long without use.
    pub idle_timeout: Duration,
    /// Sign-out after this long regardless of use.
    pub absolute_duration: Duration,
    /// Also check passwords against the online breach service.
    pub online_breach_check: bool,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            idle_timeout: IDLE_DEFAULT,
            absolute_duration: ABSOLUTE_DEFAULT,
            online_breach_check: false,
        }
    }
}

impl SecuritySettings {
    /// Validate against the ranges (BR-10.1).
    ///
    /// # Errors
    /// [`SettingsError`] for the first problem found.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.idle_timeout < IDLE_MIN || self.idle_timeout > IDLE_MAX {
            return Err(SettingsError::IdleOutOfRange);
        }
        if self.absolute_duration < ABSOLUTE_MIN || self.absolute_duration > ABSOLUTE_MAX {
            return Err(SettingsError::AbsoluteOutOfRange);
        }
        if self.idle_timeout > self.absolute_duration {
            return Err(SettingsError::IdleExceedsAbsolute);
        }
        Ok(())
    }

    /// The stored key and value pairs.
    pub fn to_stored(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                KEY_IDLE_TIMEOUT.to_owned(),
                self.idle_timeout.whole_seconds().to_string(),
            ),
            (
                KEY_ABSOLUTE_DURATION.to_owned(),
                self.absolute_duration.whole_seconds().to_string(),
            ),
            (
                KEY_ONLINE_BREACH_CHECK.to_owned(),
                self.online_breach_check.to_string(),
            ),
        ])
    }

    /// Read stored values; a missing or malformed value falls back to its default (BR-10.2).
    pub fn from_stored(stored: &BTreeMap<String, String>) -> Self {
        let defaults = Self::default();
        let seconds = |key: &str, fallback: Duration| {
            stored
                .get(key)
                .and_then(|value| value.parse::<i64>().ok())
                .map_or(fallback, Duration::seconds)
        };
        let candidate = Self {
            idle_timeout: seconds(KEY_IDLE_TIMEOUT, defaults.idle_timeout),
            absolute_duration: seconds(KEY_ABSOLUTE_DURATION, defaults.absolute_duration),
            online_breach_check: stored
                .get(KEY_ONLINE_BREACH_CHECK)
                .and_then(|value| value.parse::<bool>().ok())
                .unwrap_or(defaults.online_breach_check),
        };
        if candidate.validate().is_ok() {
            candidate
        } else {
            defaults
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_10_2_defaults_are_12_hours_14_days_and_online_check_off() {
        let defaults = SecuritySettings::default();
        assert_eq!(defaults.idle_timeout, Duration::hours(12));
        assert_eq!(defaults.absolute_duration, Duration::days(14));
        assert!(!defaults.online_breach_check);
        assert!(defaults.validate().is_ok());
        assert_eq!(SecuritySettings::from_stored(&BTreeMap::new()), defaults);
    }

    #[test]
    fn br_10_1_idle_timeout_range_is_15_minutes_to_30_days() {
        let mut s = SecuritySettings {
            absolute_duration: Duration::days(90),
            ..SecuritySettings::default()
        };
        s.idle_timeout = Duration::minutes(14);
        assert_eq!(s.validate(), Err(SettingsError::IdleOutOfRange));
        s.idle_timeout = Duration::minutes(15);
        assert_eq!(s.validate(), Ok(()));
        s.idle_timeout = Duration::days(30);
        assert_eq!(s.validate(), Ok(()));
        s.idle_timeout = Duration::days(30) + Duration::seconds(1);
        assert_eq!(s.validate(), Err(SettingsError::IdleOutOfRange));
    }

    #[test]
    fn br_10_1_absolute_range_is_1_hour_to_90_days() {
        let mut s = SecuritySettings {
            idle_timeout: Duration::minutes(15),
            ..SecuritySettings::default()
        };
        s.absolute_duration = Duration::minutes(59);
        assert_eq!(s.validate(), Err(SettingsError::AbsoluteOutOfRange));
        s.absolute_duration = Duration::days(90) + Duration::seconds(1);
        assert_eq!(s.validate(), Err(SettingsError::AbsoluteOutOfRange));
        s.absolute_duration = Duration::days(90);
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn br_10_1_the_idle_timeout_cannot_exceed_the_absolute_duration() {
        let s = SecuritySettings {
            idle_timeout: Duration::hours(3),
            absolute_duration: Duration::hours(2),
            online_breach_check: false,
        };
        assert_eq!(s.validate(), Err(SettingsError::IdleExceedsAbsolute));
    }

    #[test]
    fn stored_values_round_trip_and_malformed_ones_fall_back_to_defaults() {
        let custom = SecuritySettings {
            idle_timeout: Duration::hours(2),
            absolute_duration: Duration::days(3),
            online_breach_check: true,
        };
        assert_eq!(SecuritySettings::from_stored(&custom.to_stored()), custom);
        let mut broken = custom.to_stored();
        broken.insert(KEY_IDLE_TIMEOUT.to_owned(), "soon".to_owned());
        assert_eq!(
            SecuritySettings::from_stored(&broken).idle_timeout,
            IDLE_DEFAULT
        );
        let mut invalid = custom.to_stored();
        invalid.insert(KEY_IDLE_TIMEOUT.to_owned(), "5".to_owned());
        assert_eq!(
            SecuritySettings::from_stored(&invalid),
            SecuritySettings::default()
        );
    }
}
