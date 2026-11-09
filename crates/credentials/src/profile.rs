//! Argon2id cost profiles and the first-start benchmark (PF-4, decision T1).

use argon2::{Algorithm, Argon2, Params, Version};
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, Instant};

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashProfile {
    /// Memory in KiB.
    pub memory_kib: u32,
    /// Iterations.
    pub iterations: u32,
    /// Parallel lanes.
    pub lanes: u32,
}

impl HashProfile {
    /// The lowest profile Tandem accepts: the OWASP minimum (19 MiB, 2 iterations, 1 lane).
    pub const MINIMUM: Self = Self {
        memory_kib: 19 * 1024,
        iterations: 2,
        lanes: 1,
    };

    /// Memory use of one hash, in MiB (rounded up).
    pub const fn memory_mib(&self) -> u32 {
        self.memory_kib.div_ceil(1024)
    }

    /// The matching `argon2` parameters.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] when the values are out of Argon2's range.
    pub fn params(&self) -> Result<Params, ProfileError> {
        Params::new(self.memory_kib, self.iterations, self.lanes, None)
            .map_err(|_| ProfileError::Invalid)
    }

    /// An Argon2id context for this profile.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`].
    pub fn argon2(&self) -> Result<Argon2<'static>, ProfileError> {
        Ok(Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            self.params()?,
        ))
    }
}

/// Why a profile could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// The text is not `m=<KiB>,t=<n>,p=<n>`.
    #[error("the hash profile is not in the form m=<KiB>,t=<iterations>,p=<lanes>")]
    Malformed,
    /// The values are outside what Argon2 allows, or below the minimum.
    #[error("the hash profile values are not valid")]
    Invalid,
}

impl fmt::Display for HashProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "m={},t={},p={}",
            self.memory_kib, self.iterations, self.lanes
        )
    }
}

impl FromStr for HashProfile {
    type Err = ProfileError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut memory = None;
        let mut iterations = None;
        let mut lanes = None;
        for part in text.split(',') {
            let (key, value) = part.split_once('=').ok_or(ProfileError::Malformed)?;
            let value: u32 = value.parse().map_err(|_| ProfileError::Malformed)?;
            match key {
                "m" => memory = Some(value),
                "t" => iterations = Some(value),
                "p" => lanes = Some(value),
                _ => return Err(ProfileError::Malformed),
            }
        }
        let profile = Self {
            memory_kib: memory.ok_or(ProfileError::Malformed)?,
            iterations: iterations.ok_or(ProfileError::Malformed)?,
            lanes: lanes.ok_or(ProfileError::Malformed)?,
        };
        profile.params()?;
        if profile.memory_kib < Self::MINIMUM.memory_kib
            || profile.iterations < Self::MINIMUM.iterations
        {
            return Err(ProfileError::Invalid);
        }
        Ok(profile)
    }
}

/// Increasing cost, one lane, never above 256 MiB (decision T1, NFR design 4).
const LADDER: [HashProfile; 7] = [
    HashProfile {
        memory_kib: 19 * 1024,
        iterations: 2,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 19 * 1024,
        iterations: 3,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 32 * 1024,
        iterations: 3,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 64 * 1024,
        iterations: 3,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 64 * 1024,
        iterations: 4,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 128 * 1024,
        iterations: 4,
        lanes: 1,
    },
    HashProfile {
        memory_kib: 256 * 1024,
        iterations: 4,
        lanes: 1,
    },
];

/// Pick the strongest profile whose single hash takes no longer than `target` on this host, never
/// below [`HashProfile::MINIMUM`]. Returns the profile and the measured time of the chosen step.
/// CPU-heavy: run on a blocking thread, once, at first start.
pub fn benchmark_profile(target: Duration) -> (HashProfile, Duration) {
    benchmark_with(target, |profile| {
        let Ok(argon2) = profile.argon2() else {
            return None;
        };
        let started = Instant::now();
        let mut out = [0_u8; 32];
        argon2
            .hash_password_into(b"benchmark password", b"benchmark salt!!", &mut out)
            .ok()?;
        Some(started.elapsed())
    })
}

fn benchmark_with(
    target: Duration,
    mut measure: impl FnMut(&HashProfile) -> Option<Duration>,
) -> (HashProfile, Duration) {
    let mut best = (HashProfile::MINIMUM, Duration::ZERO);
    for (index, profile) in LADDER.iter().enumerate() {
        let Some(elapsed) = measure(profile) else {
            break;
        };
        if index == 0 {
            best = (*profile, elapsed);
        }
        if elapsed > target {
            break;
        }
        best = (*profile, elapsed);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_round_trip_through_text_and_reject_weak_or_malformed_values() {
        let profile = HashProfile {
            memory_kib: 65536,
            iterations: 3,
            lanes: 1,
        };
        assert_eq!(profile.to_string(), "m=65536,t=3,p=1");
        assert_eq!("m=65536,t=3,p=1".parse::<HashProfile>().unwrap(), profile);
        assert_eq!(
            "m=1024,t=1,p=1".parse::<HashProfile>(),
            Err(ProfileError::Invalid),
            "below the minimum"
        );
        assert_eq!(
            "m=19456,t=1,p=1".parse::<HashProfile>(),
            Err(ProfileError::Invalid)
        );
        for bad in ["", "m=1", "m=a,t=2,p=1", "x=1,t=2,p=1", "m=19456,t=2"] {
            assert!(bad.parse::<HashProfile>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn pf_4_the_benchmark_picks_the_strongest_profile_within_the_target() {
        // Pretend cost grows linearly with the ladder position: 100 ms per step.
        let measure = |profile: &HashProfile| {
            let position = LADDER.iter().position(|p| p == profile).unwrap();
            Some(Duration::from_millis(100 * (position as u64 + 1)))
        };
        let (profile, elapsed) = benchmark_with(Duration::from_millis(250), measure);
        assert_eq!(profile, LADDER[1]);
        assert_eq!(elapsed, Duration::from_millis(200));
    }

    #[test]
    fn pf_4_a_slow_host_falls_back_to_the_minimum_profile() {
        let (profile, _) =
            benchmark_with(Duration::from_millis(250), |_| Some(Duration::from_secs(5)));
        assert_eq!(profile, HashProfile::MINIMUM);
    }

    #[test]
    fn pf_4_a_fast_host_stops_at_the_top_of_the_ladder() {
        let (profile, _) =
            benchmark_with(Duration::from_secs(60), |_| Some(Duration::from_millis(1)));
        assert_eq!(profile, *LADDER.last().unwrap());
        assert!(profile.memory_mib() <= 256);
    }

    #[test]
    fn the_real_benchmark_returns_a_valid_profile() {
        let (profile, _) = benchmark_profile(Duration::from_millis(1));
        assert!(profile.params().is_ok());
        assert_eq!(
            profile,
            HashProfile::MINIMUM,
            "a 1 ms target can only select the minimum"
        );
    }
}
