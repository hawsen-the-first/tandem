//! The sandbox policy (U2: BR-18, A1, A2, A16): the tool registry, factory defaults and ranges,
//! validation, and the rule that a narrower (session) policy may tighten but never exceed the
//! instance policy. Pure; the runner that enforces it is a later unit.

use std::collections::BTreeSet;
use std::net::IpAddr;

/// A tool an agent may be allowed to use. The registry grows with later units; unknown names are
/// not accepted anywhere (A1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolId {
    /// Read files in the session workspace.
    WorkspaceRead,
    /// Create and change files in the session workspace.
    WorkspaceWrite,
    /// Create and edit artifacts.
    ArtifactEdit,
    /// Search the web (needs a search provider).
    WebSearch,
    /// Read approved extra repositories.
    RepoRead,
    /// Run commands. Listed, but never enabled in the MVP (FR-8.3).
    CommandRun,
}

impl ToolId {
    /// Every registered tool, in the order shown.
    pub const ALL: [Self; 6] = [
        Self::WorkspaceRead,
        Self::WorkspaceWrite,
        Self::ArtifactEdit,
        Self::WebSearch,
        Self::RepoRead,
        Self::CommandRun,
    ];

    /// Stable identifier used in storage and the API.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRead => "workspace_read",
            Self::WorkspaceWrite => "workspace_write",
            Self::ArtifactEdit => "artifact_edit",
            Self::WebSearch => "web_search",
            Self::RepoRead => "repo_read",
            Self::CommandRun => "command_run",
        }
    }

    /// Parse a stable identifier.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.as_str() == text)
    }

    /// What the tool lets an agent do, in plain words.
    pub const fn description(self) -> &'static str {
        match self {
            Self::WorkspaceRead => "Read files in the session workspace",
            Self::WorkspaceWrite => "Create and change files in the session workspace",
            Self::ArtifactEdit => "Create and edit artifacts",
            Self::WebSearch => "Search the web (needs a search provider)",
            Self::RepoRead => "Read approved extra repositories",
            Self::CommandRun => "Run commands (not available in this version)",
        }
    }

    /// Whether the tool can be enabled in this version. `command_run` never can (BR-18.3).
    pub const fn can_be_enabled(self) -> bool {
        !matches!(self, Self::CommandRun)
    }
}

/// How runs are isolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerTier {
    /// Confinement inside the worker process by the tool layer.
    InProcess,
    /// A container per run (needs a reachable runtime, BR-18.2).
    Container,
}

impl RunnerTier {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Container => "container",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "in_process" => Some(Self::InProcess),
            "container" => Some(Self::Container),
            _ => None,
        }
    }

    /// The container tier isolates more than the in-process tier.
    const fn strictness(self) -> u8 {
        match self {
            Self::InProcess => 0,
            Self::Container => 1,
        }
    }
}

/// Which policy record this is. Only `Instance` exists in the MVP; `Session` is reserved so that
/// per-session overrides need no redesign (FR-16.6, BR-18.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyScope {
    /// The instance-wide policy.
    Instance,
    /// A session's own, narrower policy (post-MVP).
    Session,
}

impl PolicyScope {
    /// Stable text form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instance => "instance",
            Self::Session => "session",
        }
    }

    /// Parse the stable text form.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "instance" => Some(Self::Instance),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

/// Most hosts in a network allow-list.
pub const MAX_ALLOWED_HOSTS: usize = 50;

/// A host name in a network allow-list: lower-case ASCII, labels of letters, digits and hyphens,
/// optionally one leading `*.` wildcard over at least two labels (BR-18.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostName(String);

impl HostName {
    /// Validate and normalise a host name.
    ///
    /// # Errors
    /// [`PolicyIssue::InvalidHost`] with the reason.
    pub fn parse(text: &str) -> Result<Self, PolicyIssue> {
        let invalid = |reason: &'static str| PolicyIssue::InvalidHost {
            entry: text.trim().chars().take(80).collect(),
            reason,
        };
        let text = text.trim().to_ascii_lowercase();
        if text.is_empty() {
            return Err(invalid("Enter a host name such as api.example.com"));
        }
        if text.len() > 253 {
            return Err(invalid("A host name can be at most 253 characters"));
        }
        if !text.is_ascii() {
            return Err(invalid("Use the ASCII (punycode) form of the name"));
        }
        if text.contains("://") || text.contains(['/', ':', '@', ' ', '?', '#']) {
            return Err(invalid(
                "Enter only the host name, without a scheme, port or path",
            ));
        }
        if text.parse::<IpAddr>().is_ok()
            || text
                .split('.')
                .all(|label| label.chars().all(|c| c.is_ascii_digit()))
        {
            return Err(invalid("Enter a host name, not an address"));
        }
        let (wildcard, rest) = match text.strip_prefix("*.") {
            Some(rest) => (true, rest),
            None => (false, text.as_str()),
        };
        let labels: Vec<&str> = rest.split('.').collect();
        if wildcard && labels.len() < 2 {
            return Err(invalid(
                "A wildcard needs at least two labels, such as *.example.com",
            ));
        }
        for label in &labels {
            let well_formed = !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            if !well_formed {
                return Err(invalid(
                    "Names use letters, digits and hyphens in labels of up to 63 characters",
                ));
            }
        }
        Ok(Self(text))
    }

    /// The normalised name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `other` is allowed by this entry as a ceiling: the same name, or covered by this
    /// entry's wildcard (`*.example.com` covers `a.example.com`, `*.a.example.com` and itself).
    fn covers(&self, other: &Self) -> bool {
        if self.0 == other.0 {
            return true;
        }
        let Some(suffix) = self.0.strip_prefix("*") else {
            return false;
        };
        let target = other.0.strip_prefix('*').unwrap_or(&other.0);
        // `suffix` is ".example.com"; `target` is "a.example.com" or ".a.example.com".
        target.ends_with(suffix)
    }
}

/// The network rule (BR-18.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkRule {
    /// No network access (the default).
    None,
    /// Only these hosts.
    AllowHosts(BTreeSet<HostName>),
}

impl NetworkRule {
    /// Build an allow-list from entries: normalised, de-duplicated, at most [`MAX_ALLOWED_HOSTS`].
    ///
    /// # Errors
    /// Every [`PolicyIssue`] found (all bad entries are reported, not just the first).
    pub fn allow_hosts<I, S>(entries: I) -> Result<Self, Vec<PolicyIssue>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut hosts = BTreeSet::new();
        let mut issues = Vec::new();
        for entry in entries {
            match HostName::parse(entry.as_ref()) {
                Ok(host) => {
                    hosts.insert(host);
                }
                Err(issue) => issues.push(issue),
            }
        }
        if hosts.len() > MAX_ALLOWED_HOSTS {
            issues.push(PolicyIssue::TooManyHosts {
                max: MAX_ALLOWED_HOSTS,
            });
        }
        if issues.is_empty() {
            Ok(Self::AllowHosts(hosts))
        } else {
            Err(issues)
        }
    }
}

/// An allowed range for a limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Smallest allowed value.
    pub min: u64,
    /// Largest allowed value.
    pub max: u64,
    /// The factory default.
    pub default: u64,
}

impl Range {
    const fn contains(self, value: u64) -> bool {
        value >= self.min && value <= self.max
    }
}

const KIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;

/// Wall-clock seconds per run (A2).
pub const WALL_CLOCK_SECONDS: Range = Range {
    min: 30,
    max: 3_600,
    default: 600,
};
/// Output bytes per run.
pub const MAX_OUTPUT_BYTES: Range = Range {
    min: 64 * KIB,
    max: 16 * MIB,
    default: MIB,
};
/// Largest single file written.
pub const MAX_FILE_BYTES: Range = Range {
    min: 64 * KIB,
    max: 100 * MIB,
    default: 5 * MIB,
};
/// Files written per run.
pub const MAX_FILES_WRITTEN: Range = Range {
    min: 1,
    max: 10_000,
    default: 200,
};
/// Container memory in MiB (enforced by the container runner only).
pub const MEMORY_MEBIBYTES: Range = Range {
    min: 64,
    max: 16_384,
    default: 512,
};
/// Container CPU in millicores (enforced by the container runner only).
pub const CPU_MILLICORES: Range = Range {
    min: 100,
    max: 8_000,
    default: 1_000,
};

/// Resource limits (BR-18.5). The factory defaults are only starting values: a stored value always
/// wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Wall-clock seconds per run.
    pub wall_clock_seconds: u64,
    /// Output bytes per run.
    pub max_output_bytes: u64,
    /// Largest single file written, in bytes.
    pub max_file_bytes: u64,
    /// Files written per run.
    pub max_files_written: u64,
    /// Container memory, MiB.
    pub memory_mebibytes: u64,
    /// Container CPU, millicores.
    pub cpu_millicores: u64,
}

impl Limits {
    /// The factory defaults (A2).
    pub const fn factory() -> Self {
        Self {
            wall_clock_seconds: WALL_CLOCK_SECONDS.default,
            max_output_bytes: MAX_OUTPUT_BYTES.default,
            max_file_bytes: MAX_FILE_BYTES.default,
            max_files_written: MAX_FILES_WRITTEN.default,
            memory_mebibytes: MEMORY_MEBIBYTES.default,
            cpu_millicores: CPU_MILLICORES.default,
        }
    }

    fn fields(&self) -> [(&'static str, u64, Range); 6] {
        [
            (
                "wall_clock_seconds",
                self.wall_clock_seconds,
                WALL_CLOCK_SECONDS,
            ),
            ("max_output_bytes", self.max_output_bytes, MAX_OUTPUT_BYTES),
            ("max_file_bytes", self.max_file_bytes, MAX_FILE_BYTES),
            (
                "max_files_written",
                self.max_files_written,
                MAX_FILES_WRITTEN,
            ),
            ("memory_mebibytes", self.memory_mebibytes, MEMORY_MEBIBYTES),
            ("cpu_millicores", self.cpu_millicores, CPU_MILLICORES),
        ]
    }

    /// Every limit no larger than the same limit in `ceiling`.
    fn min_with(&self, ceiling: &Self) -> Self {
        Self {
            wall_clock_seconds: self.wall_clock_seconds.min(ceiling.wall_clock_seconds),
            max_output_bytes: self.max_output_bytes.min(ceiling.max_output_bytes),
            max_file_bytes: self.max_file_bytes.min(ceiling.max_file_bytes),
            max_files_written: self.max_files_written.min(ceiling.max_files_written),
            memory_mebibytes: self.memory_mebibytes.min(ceiling.memory_mebibytes),
            cpu_millicores: self.cpu_millicores.min(ceiling.cpu_millicores),
        }
    }
}

/// A problem with a policy, shown next to its field.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyIssue {
    /// `command_run` was enabled.
    #[error("command_run cannot be enabled in this version")]
    CommandRunNotAllowed,
    /// A network entry is not a valid host name.
    #[error("{reason}")]
    InvalidHost {
        /// What was entered (shortened).
        entry: String,
        /// Why it was refused.
        reason: &'static str,
    },
    /// Too many network entries.
    #[error("The network allow-list is limited to {max} hosts")]
    TooManyHosts {
        /// The limit.
        max: usize,
    },
    /// A limit outside its range.
    #[error("{field} must be between {min} and {max}")]
    OutOfRange {
        /// The limit's name.
        field: &'static str,
        /// Smallest allowed.
        min: u64,
        /// Largest allowed.
        max: u64,
    },
    /// A narrower policy tried to go above the instance policy (BR-18.8).
    #[error("{field} cannot be more permissive than the instance policy")]
    ExceedsCeiling {
        /// The setting.
        field: &'static str,
    },
}

impl PolicyIssue {
    /// The API field the problem belongs to.
    pub fn field(&self) -> &'static str {
        match self {
            Self::CommandRunNotAllowed => "tool_allow_list",
            Self::InvalidHost { .. } | Self::TooManyHosts { .. } => "network",
            Self::OutOfRange { field, .. } | Self::ExceedsCeiling { field } => field,
        }
    }
}

/// A sandbox policy record (BR-18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Which record.
    pub scope: PolicyScope,
    /// How runs are isolated.
    pub runner_tier: RunnerTier,
    /// Tools agents may use.
    pub tool_allow_list: BTreeSet<ToolId>,
    /// The network rule.
    pub network: NetworkRule,
    /// Resource limits.
    pub limits: Limits,
    /// Increases with every saved change; a run records the version it captured (D5).
    pub version: u32,
}

impl SandboxPolicy {
    /// The policy of a new instance: in-process, every enableable tool, no network, factory limits.
    pub fn factory_default() -> Self {
        Self {
            scope: PolicyScope::Instance,
            runner_tier: RunnerTier::InProcess,
            tool_allow_list: ToolId::ALL
                .into_iter()
                .filter(|tool| tool.can_be_enabled())
                .collect(),
            network: NetworkRule::None,
            limits: Limits::factory(),
            version: 1,
        }
    }

    /// Check the whole policy (BR-18.3 to BR-18.6) and report every problem.
    ///
    /// # Errors
    /// Every [`PolicyIssue`] found.
    pub fn validate(&self) -> Result<(), Vec<PolicyIssue>> {
        let mut issues = Vec::new();
        if self.tool_allow_list.contains(&ToolId::CommandRun) {
            issues.push(PolicyIssue::CommandRunNotAllowed);
        }
        if let NetworkRule::AllowHosts(hosts) = &self.network
            && hosts.len() > MAX_ALLOWED_HOSTS
        {
            issues.push(PolicyIssue::TooManyHosts {
                max: MAX_ALLOWED_HOSTS,
            });
        }
        for (field, value, range) in self.limits.fields() {
            if !range.contains(value) {
                issues.push(PolicyIssue::OutOfRange {
                    field,
                    min: range.min,
                    max: range.max,
                });
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    /// Where this policy goes beyond `ceiling` (BR-18.8): any limit above it, a tool the ceiling
    /// does not allow, a looser network rule, a looser runner tier. Empty means within the ceiling.
    pub fn exceeds(&self, ceiling: &Self) -> Vec<PolicyIssue> {
        let mut issues = Vec::new();
        for ((field, value, _), (_, limit, _)) in self
            .limits
            .fields()
            .into_iter()
            .zip(ceiling.limits.fields())
        {
            if value > limit {
                issues.push(PolicyIssue::ExceedsCeiling { field });
            }
        }
        if !self.tool_allow_list.is_subset(&ceiling.tool_allow_list) {
            issues.push(PolicyIssue::ExceedsCeiling {
                field: "tool_allow_list",
            });
        }
        if self.runner_tier.strictness() < ceiling.runner_tier.strictness() {
            issues.push(PolicyIssue::ExceedsCeiling {
                field: "runner_tier",
            });
        }
        let network_ok = match (&self.network, &ceiling.network) {
            (NetworkRule::None, _) => true,
            (NetworkRule::AllowHosts(_), NetworkRule::None) => false,
            (NetworkRule::AllowHosts(own), NetworkRule::AllowHosts(allowed)) => own
                .iter()
                .all(|host| allowed.iter().any(|entry| entry.covers(host))),
        };
        if !network_ok {
            issues.push(PolicyIssue::ExceedsCeiling { field: "network" });
        }
        issues
    }

    /// This policy limited to `ceiling`: smaller limits, fewer tools, a no wider network rule, the
    /// stricter runner tier. Used when a run starts, so lowering the instance policy later takes
    /// effect on existing session values (BR-18.8). Everything else is this policy's.
    #[must_use]
    pub fn clamped_to(&self, ceiling: &Self) -> Self {
        let network = match (&self.network, &ceiling.network) {
            (NetworkRule::None, _) | (_, NetworkRule::None) => NetworkRule::None,
            (NetworkRule::AllowHosts(own), NetworkRule::AllowHosts(allowed)) => {
                NetworkRule::AllowHosts(
                    own.iter()
                        .filter(|host| allowed.iter().any(|entry| entry.covers(host)))
                        .cloned()
                        .collect(),
                )
            }
        };
        Self {
            scope: self.scope,
            runner_tier: if self.runner_tier.strictness() >= ceiling.runner_tier.strictness() {
                self.runner_tier
            } else {
                ceiling.runner_tier
            },
            tool_allow_list: self
                .tool_allow_list
                .intersection(&ceiling.tool_allow_list)
                .copied()
                .collect(),
            network,
            limits: self.limits.min_with(&ceiling.limits),
            version: self.version,
        }
    }
}

#[cfg(test)]
mod tests;
