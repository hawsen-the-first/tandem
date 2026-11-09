//! Background jobs and their retry policy (BR-13.3).

use crate::ids::{ChatMessageId, JobId};
use time::{Duration, OffsetDateTime};

/// Total attempts: the first try plus three retries.
pub const MAX_ATTEMPTS: u32 = 4;
/// Delay before retry 1, 2 and 3.
pub const RETRY_DELAYS: [Duration; 3] = [
    Duration::seconds(5),
    Duration::seconds(30),
    Duration::minutes(2),
];

/// What a job does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobKind {
    /// Reply to a skeleton message with the scripted mock agent.
    MockAgentReply {
        /// The message to reply to.
        message_id: ChatMessageId,
    },
}

impl JobKind {
    /// Stable kind name used in storage and metrics.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::MockAgentReply { .. } => "mock_agent_reply",
        }
    }
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Waiting to be leased.
    Queued,
    /// Being worked on.
    Leased,
    /// Finished successfully.
    Done,
    /// Gave up after the last attempt.
    Failed,
}

/// What a failure led to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureOutcome {
    /// It will run again at the given time.
    RetryAt(OffsetDateTime),
    /// No attempts remain.
    Failed,
}

/// Why a job transition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the job is {0:?} and cannot make that transition")]
pub struct JobTransitionError(pub JobStatus);

/// A queued unit of background work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// Identifier.
    pub id: JobId,
    /// What to do.
    pub kind: JobKind,
    /// Current status.
    pub status: JobStatus,
    /// Attempts started so far.
    pub attempts: u32,
    /// Earliest time it may run.
    pub run_at: OffsetDateTime,
    /// The last error, for diagnosis.
    pub last_error: Option<String>,
    /// While leased, when the lease lapses unless renewed.
    pub lease_expires_at: Option<OffsetDateTime>,
    /// Trace context carried from the originating request.
    pub trace_context: Option<String>,
}

impl Job {
    /// A new queued job.
    pub fn new(
        id: JobId,
        kind: JobKind,
        run_at: OffsetDateTime,
        trace_context: Option<String>,
    ) -> Self {
        Self {
            id,
            kind,
            status: JobStatus::Queued,
            attempts: 0,
            run_at,
            last_error: None,
            lease_expires_at: None,
            trace_context,
        }
    }

    /// Lease the job for `ttl`; this starts an attempt.
    ///
    /// # Errors
    /// [`JobTransitionError`] unless the job is queued and due.
    pub fn lease(&mut self, now: OffsetDateTime, ttl: Duration) -> Result<(), JobTransitionError> {
        if self.status != JobStatus::Queued || self.run_at > now {
            return Err(JobTransitionError(self.status));
        }
        self.status = JobStatus::Leased;
        self.attempts += 1;
        self.lease_expires_at = Some(now + ttl);
        Ok(())
    }

    /// Renew a lease.
    ///
    /// # Errors
    /// [`JobTransitionError`] unless the job is leased.
    pub fn heartbeat(
        &mut self,
        now: OffsetDateTime,
        ttl: Duration,
    ) -> Result<(), JobTransitionError> {
        if self.status != JobStatus::Leased {
            return Err(JobTransitionError(self.status));
        }
        self.lease_expires_at = Some(now + ttl);
        Ok(())
    }

    /// Mark the job done.
    ///
    /// # Errors
    /// [`JobTransitionError`] unless the job is leased.
    pub fn complete(&mut self) -> Result<(), JobTransitionError> {
        if self.status != JobStatus::Leased {
            return Err(JobTransitionError(self.status));
        }
        self.status = JobStatus::Done;
        self.lease_expires_at = None;
        Ok(())
    }

    /// Record a failed attempt: retry later, or give up after the last attempt.
    ///
    /// # Errors
    /// [`JobTransitionError`] unless the job is leased.
    pub fn fail(
        &mut self,
        now: OffsetDateTime,
        error: &str,
    ) -> Result<FailureOutcome, JobTransitionError> {
        if self.status != JobStatus::Leased {
            return Err(JobTransitionError(self.status));
        }
        self.last_error = Some(error.to_owned());
        self.lease_expires_at = None;
        Ok(self.schedule_after_failed_attempt(now))
    }

    /// A lapsed lease counts as a failed attempt (a crashed worker). Returns `None` if the lease is
    /// still valid or the job is not leased.
    pub fn recover_if_lease_lapsed(&mut self, now: OffsetDateTime) -> Option<FailureOutcome> {
        if self.status != JobStatus::Leased || self.lease_expires_at.is_none_or(|until| until > now)
        {
            return None;
        }
        self.last_error = Some("lease expired".to_owned());
        self.lease_expires_at = None;
        Some(self.schedule_after_failed_attempt(now))
    }

    fn schedule_after_failed_attempt(&mut self, now: OffsetDateTime) -> FailureOutcome {
        let retry_index = usize::try_from(self.attempts.saturating_sub(1)).unwrap_or(usize::MAX);
        match RETRY_DELAYS.get(retry_index) {
            Some(delay) if self.attempts < MAX_ATTEMPTS => {
                self.status = JobStatus::Queued;
                self.run_at = now + *delay;
                FailureOutcome::RetryAt(self.run_at)
            }
            _ => {
                self.status = JobStatus::Failed;
                FailureOutcome::Failed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tandem_support::{FakeClock, FakeRng};

    fn job(now: OffsetDateTime) -> Job {
        let clock = FakeClock::new(now);
        let rng = FakeRng::new(8);
        Job::new(
            JobId::generate(&clock, &rng).unwrap(),
            JobKind::MockAgentReply {
                message_id: ChatMessageId::generate(&clock, &rng).unwrap(),
            },
            now,
            None,
        )
    }

    #[test]
    fn br_13_3_a_failing_job_retries_after_5s_30s_and_2min_then_fails() {
        let mut now = OffsetDateTime::UNIX_EPOCH;
        let mut j = job(now);
        let expected = [
            Duration::seconds(5),
            Duration::seconds(30),
            Duration::minutes(2),
        ];
        for delay in expected {
            j.lease(now, Duration::seconds(60)).unwrap();
            assert_eq!(
                j.fail(now, "boom").unwrap(),
                FailureOutcome::RetryAt(now + delay)
            );
            assert_eq!(j.status, JobStatus::Queued);
            now += delay;
        }
        j.lease(now, Duration::seconds(60)).unwrap();
        assert_eq!(j.attempts, 4);
        assert_eq!(j.fail(now, "boom").unwrap(), FailureOutcome::Failed);
        assert_eq!(j.status, JobStatus::Failed);
        assert_eq!(j.last_error.as_deref(), Some("boom"));
    }

    #[test]
    fn a_job_cannot_be_leased_before_it_is_due_or_twice() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut j = job(now);
        j.run_at = now + Duration::seconds(10);
        assert!(j.lease(now, Duration::seconds(60)).is_err());
        assert!(
            j.lease(now + Duration::seconds(10), Duration::seconds(60))
                .is_ok()
        );
        assert!(
            j.lease(now + Duration::seconds(11), Duration::seconds(60))
                .is_err()
        );
    }

    #[test]
    fn heartbeat_renews_the_lease_and_completion_ends_it() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut j = job(now);
        j.lease(now, Duration::seconds(60)).unwrap();
        j.heartbeat(now + Duration::seconds(30), Duration::seconds(60))
            .unwrap();
        assert_eq!(j.lease_expires_at, Some(now + Duration::seconds(90)));
        j.complete().unwrap();
        assert_eq!(j.status, JobStatus::Done);
        assert!(j.complete().is_err());
    }

    #[test]
    fn br_13_3_a_crashed_worker_lease_lapses_and_counts_as_a_failed_attempt() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut j = job(now);
        j.lease(now, Duration::seconds(60)).unwrap();
        assert_eq!(j.recover_if_lease_lapsed(now + Duration::seconds(59)), None);
        let outcome = j
            .recover_if_lease_lapsed(now + Duration::seconds(61))
            .unwrap();
        assert_eq!(
            outcome,
            FailureOutcome::RetryAt(now + Duration::seconds(61) + Duration::seconds(5))
        );
        assert_eq!(j.last_error.as_deref(), Some("lease expired"));
    }

    #[test]
    fn kind_names_are_stable() {
        assert_eq!(
            job(OffsetDateTime::UNIX_EPOCH).kind.name(),
            "mock_agent_reply"
        );
    }
}
