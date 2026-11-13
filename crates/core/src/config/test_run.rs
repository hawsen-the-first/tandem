//! Test runs: what a test action records (BR-14.6, BR-14.7, A4; NFR design pattern 2).

use super::connectors::{ConnectorKind, SearchKind};
use super::provider::ToolCalling;
use crate::ids::{ProviderId, ProviderModelId, TestRunId, UserId};
use time::OffsetDateTime;

/// Most repository names kept in a test result (BR-17.3).
pub const MAX_REPOSITORIES_LISTED: usize = 100;

/// What a test targets. Also the key of "one active run per target" and of the advisory lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestTarget {
    /// A model provider and its models.
    Provider(ProviderId),
    /// A search provider.
    Search(SearchKind),
    /// A connector.
    Connector(ConnectorKind),
    /// The container runtime.
    Runtime,
}

impl TestTarget {
    /// The kind name used in metrics and logs (never a host or an id).
    pub const fn kind_name(self) -> &'static str {
        match self {
            Self::Provider(_) => "provider",
            Self::Search(_) => "search",
            Self::Connector(_) => "github",
            Self::Runtime => "runtime",
        }
    }

    /// A stable text key: `provider:<id>`, `search:<kind>`, `connector:<kind>` or `runtime`.
    pub fn key(self) -> String {
        match self {
            Self::Provider(id) => format!("provider:{id}"),
            Self::Search(kind) => format!("search:{}", kind.as_str()),
            Self::Connector(kind) => format!("connector:{}", kind.as_str()),
            Self::Runtime => "runtime".to_owned(),
        }
    }

    /// Parse a key produced by [`Self::key`].
    pub fn parse_key(text: &str) -> Option<Self> {
        if text == "runtime" {
            return Some(Self::Runtime);
        }
        let (kind, rest) = text.split_once(':')?;
        match kind {
            "provider" => rest.parse().ok().map(Self::Provider),
            "search" => SearchKind::parse(rest).map(Self::Search),
            "connector" => ConnectorKind::parse(rest).map(Self::Connector),
            _ => None,
        }
    }
}

/// The outcome of one probe. The text shown is always fixed (SE-22): remote text is never relayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    /// It worked.
    Ok,
    /// Could not connect.
    Unreachable,
    /// Took too long.
    Timeout,
    /// The server refused the key or token.
    Unauthorised,
    /// The provider does not know the model name.
    UnknownModel,
    /// The answer was not understood, was too large or redirected somewhere not allowed.
    BadResponse,
    /// The address is not allowed (BR-19).
    RefusedAddress,
    /// Nothing is configured to test.
    NotConfigured,
}

impl TestStatus {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unreachable => "unreachable",
            Self::Timeout => "timeout",
            Self::Unauthorised => "unauthorised",
            Self::UnknownModel => "unknown_model",
            Self::BadResponse => "bad_response",
            Self::RefusedAddress => "refused_address",
            Self::NotConfigured => "not_configured",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Ok,
            Self::Unreachable,
            Self::Timeout,
            Self::Unauthorised,
            Self::UnknownModel,
            Self::BadResponse,
            Self::RefusedAddress,
            Self::NotConfigured,
        ]
        .into_iter()
        .find(|status| status.as_str() == text)
    }

    /// The fixed, safe message for the admin.
    pub const fn message(self) -> &'static str {
        match self {
            Self::Ok => "It worked.",
            Self::Unreachable => "The server could not be reached.",
            Self::Timeout => "The server took too long to answer.",
            Self::Unauthorised => "The server refused the key or token.",
            Self::UnknownModel => "The provider does not know this model name.",
            Self::BadResponse => "The server's answer was not what was expected.",
            Self::RefusedAddress => "That address is not allowed.",
            Self::NotConfigured => "Nothing is set up to test.",
        }
    }
}

/// What a probe looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSubject {
    /// A provider as a whole (reachability and authorisation).
    Provider,
    /// One model of a provider.
    Model(ProviderModelId),
    /// The search provider.
    Search,
    /// The connector.
    Connector,
    /// The container runtime.
    Runtime,
}

impl ProbeSubject {
    /// A stable text key: `provider`, `model:<id>`, `search`, `connector` or `runtime`.
    pub fn key(self) -> String {
        match self {
            Self::Provider => "provider".to_owned(),
            Self::Model(id) => format!("model:{id}"),
            Self::Search => "search".to_owned(),
            Self::Connector => "connector".to_owned(),
            Self::Runtime => "runtime".to_owned(),
        }
    }

    /// Parse a key produced by [`Self::key`].
    pub fn parse_key(text: &str) -> Option<Self> {
        match text {
            "provider" => Some(Self::Provider),
            "search" => Some(Self::Search),
            "connector" => Some(Self::Connector),
            "runtime" => Some(Self::Runtime),
            _ => text
                .strip_prefix("model:")
                .and_then(|id| id.parse().ok())
                .map(Self::Model),
        }
    }
}

/// A fixed note that may accompany a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeNote {
    /// The search worked but returned nothing (BR-16.4).
    NoResults,
}

impl ProbeNote {
    /// The fixed text.
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoResults => "The search worked but returned no results for the test query.",
        }
    }
}

/// Small structured facts a probe may learn. Never secrets and never remote text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProbeDetails {
    /// A model's tool calling.
    pub tool_calling: Option<ToolCalling>,
    /// A model's context length.
    pub context_tokens: Option<u32>,
    /// The GitHub account the token belongs to.
    pub account_login: Option<String>,
    /// When the GitHub token expires.
    pub token_expires_at: Option<OffsetDateTime>,
    /// Up to [`MAX_REPOSITORIES_LISTED`] reachable repository names (shown in the result only).
    pub repositories: Vec<String>,
    /// How many repositories are reachable in total.
    pub repository_total: Option<u32>,
    /// The container runtime's name.
    pub runtime_name: Option<String>,
    /// The container runtime's version.
    pub runtime_version: Option<String>,
}

impl ProbeDetails {
    /// Record the repositories found, keeping at most the listing limit.
    pub fn with_repositories(mut self, mut names: Vec<String>, total: u32) -> Self {
        names.truncate(MAX_REPOSITORIES_LISTED);
        self.repositories = names;
        self.repository_total = Some(total);
        self
    }
}

/// One probe's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    /// What was probed.
    pub subject: ProbeSubject,
    /// The outcome.
    pub status: TestStatus,
    /// A fixed note, if any.
    pub note: Option<ProbeNote>,
    /// Facts learned.
    pub details: ProbeDetails,
}

impl ProbeResult {
    /// A result with no details.
    pub fn new(subject: ProbeSubject, status: TestStatus) -> Self {
        Self {
            subject,
            status,
            note: None,
            details: ProbeDetails::default(),
        }
    }

    /// The fixed message for the status.
    pub const fn message(&self) -> &'static str {
        self.status.message()
    }
}

/// Where a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// Waiting for a worker.
    Queued,
    /// A worker is running it.
    Running,
    /// It finished (the probes may still report problems).
    Succeeded,
    /// The run itself could not complete.
    Failed,
}

impl RunStatus {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Queued, Self::Running, Self::Succeeded, Self::Failed]
            .into_iter()
            .find(|status| status.as_str() == text)
    }

    /// Queued or running: the states a target allows only one run in.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

/// Why a run failed as a whole. The text is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    /// The job crashed on its last attempt.
    CouldNotComplete,
    /// No worker picked it up in time.
    NoWorkerAvailable,
}

impl FailureReason {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CouldNotComplete => "could_not_complete",
            Self::NoWorkerAvailable => "no_worker_available",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        [Self::CouldNotComplete, Self::NoWorkerAvailable]
            .into_iter()
            .find(|reason| reason.as_str() == text)
    }

    /// The fixed message.
    pub const fn message(self) -> &'static str {
        match self {
            Self::CouldNotComplete => "The test could not complete.",
            Self::NoWorkerAvailable => "No worker is available to run the test.",
        }
    }
}

/// A transition that is not allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a run that is {from:?} cannot {action}")]
pub struct RunError {
    /// The state it was in.
    pub from: RunStatus,
    /// What was attempted.
    pub action: &'static str,
}

/// A test run: queued by an admin, run by a worker, kept for display (D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    /// Identifier.
    pub id: TestRunId,
    /// What is tested.
    pub target: TestTarget,
    /// Where the run is.
    pub status: RunStatus,
    /// Who started it.
    pub requested_by: UserId,
    /// When it was queued.
    pub requested_at: OffsetDateTime,
    /// When a worker started it.
    pub started_at: Option<OffsetDateTime>,
    /// When it finished.
    pub finished_at: Option<OffsetDateTime>,
    /// Probe results so far, in arrival order.
    pub results: Vec<ProbeResult>,
    /// Why the run failed, when it did.
    pub failure: Option<FailureReason>,
}

impl TestRun {
    /// A new run, queued.
    pub fn queued(
        id: TestRunId,
        target: TestTarget,
        requested_by: UserId,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            target,
            status: RunStatus::Queued,
            requested_by,
            requested_at: now,
            started_at: None,
            finished_at: None,
            results: Vec::new(),
            failure: None,
        }
    }

    fn refuse(&self, action: &'static str) -> RunError {
        RunError {
            from: self.status,
            action,
        }
    }

    /// A worker takes the run.
    ///
    /// # Errors
    /// [`RunError`] unless the run is queued.
    pub fn start(&mut self, now: OffsetDateTime) -> Result<(), RunError> {
        if self.status != RunStatus::Queued {
            return Err(self.refuse("start"));
        }
        self.status = RunStatus::Running;
        self.started_at = Some(now);
        Ok(())
    }

    /// Store a probe result, replacing an earlier one for the same subject.
    ///
    /// # Errors
    /// [`RunError`] unless the run is running.
    pub fn record(&mut self, result: ProbeResult) -> Result<(), RunError> {
        if self.status != RunStatus::Running {
            return Err(self.refuse("record a result"));
        }
        match self
            .results
            .iter_mut()
            .find(|existing| existing.subject == result.subject)
        {
            Some(existing) => *existing = result,
            None => self.results.push(result),
        }
        Ok(())
    }

    /// The run finished; the probes may still hold problems.
    ///
    /// # Errors
    /// [`RunError`] unless the run is running.
    pub fn finish(&mut self, now: OffsetDateTime) -> Result<(), RunError> {
        if self.status != RunStatus::Running {
            return Err(self.refuse("finish"));
        }
        self.status = RunStatus::Succeeded;
        self.finished_at = Some(now);
        Ok(())
    }

    /// The run could not complete.
    ///
    /// # Errors
    /// [`RunError`] unless the run is queued or running.
    pub fn fail(&mut self, now: OffsetDateTime, reason: FailureReason) -> Result<(), RunError> {
        if !self.status.is_active() {
            return Err(self.refuse("fail"));
        }
        self.status = RunStatus::Failed;
        self.finished_at = Some(now);
        self.failure = Some(reason);
        Ok(())
    }

    /// The queue is retrying the job: back to queued with no partial results (D2).
    ///
    /// # Errors
    /// [`RunError`] unless the run is running.
    pub fn requeue(&mut self) -> Result<(), RunError> {
        if self.status != RunStatus::Running {
            return Err(self.refuse("be re-queued"));
        }
        self.status = RunStatus::Queued;
        self.started_at = None;
        self.results.clear();
        Ok(())
    }

    /// Whether the run still occupies its target.
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }
}
