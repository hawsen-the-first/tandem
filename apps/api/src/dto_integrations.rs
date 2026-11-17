//! Bodies for instance administration (U2). Secrets are write-only: requests carry them, responses
//! say only whether one is saved and when (BR-20.2). Remote text never appears in a result.

use crate::dto::timestamp;
use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use tandem_app::{
    AppError, ConnectorView, DefaultChoice, DefaultsView, FieldIssue, ModelInput, ModelView,
    ProviderInput, ProviderView, RemovalImpact, SearchActivation, SearchInput, SearchOverview,
    SearchView, SecretInput,
};
use tandem_core::config::{
    DefaultRole, FailureReason, ProbeResult, ProbeSubject, SearchKind, SuitabilityWarning, TestRun,
    ToolCalling,
};
use tandem_core::ids::ProviderModelId;
use tandem_core::sandbox_policy::{
    self, HostName, Limits, NetworkRule, PolicyScope, RunnerTier, SandboxPolicy, ToolId,
};
use utoipa::ToSchema;

// ---- test runs ----------------------------------------------------------------------------

/// Facts a probe learned. Never secrets and never text from the remote server.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProbeDetailsDto {
    /// A model's tool calling: `supported`, `unsupported` or `unknown`.
    pub tool_calling: Option<String>,
    /// A model's context length in tokens.
    pub context_tokens: Option<u32>,
    /// The GitHub account the token belongs to.
    pub account_login: Option<String>,
    /// When the GitHub token expires, if GitHub says.
    pub token_expires_at: Option<String>,
    /// Up to 100 repositories the token can reach.
    pub repositories: Vec<String>,
    /// How many repositories the token can reach.
    pub repository_total: Option<u32>,
    /// The container runtime's name.
    pub runtime_name: Option<String>,
    /// The container runtime's version.
    pub runtime_version: Option<String>,
}

/// One probe's result.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProbeResultDto {
    /// `provider`, `model`, `search`, `connector` or `runtime`.
    pub subject: String,
    /// The model, when `subject` is `model`.
    pub model_id: Option<String>,
    /// `ok`, `unreachable`, `timeout`, `unauthorised`, `unknown_model`, `bad_response`,
    /// `refused_address` or `not_configured`.
    pub status: String,
    /// A fixed, safe sentence for the status.
    pub message: String,
    /// A fixed note, such as `no_results`.
    pub note: Option<String>,
    /// Facts learned.
    pub details: ProbeDetailsDto,
}

impl From<&ProbeResult> for ProbeResultDto {
    fn from(result: &ProbeResult) -> Self {
        let (subject, model_id) = match result.subject {
            ProbeSubject::Provider => ("provider", None),
            ProbeSubject::Model(id) => ("model", Some(id.to_string())),
            ProbeSubject::Search => ("search", None),
            ProbeSubject::Connector => ("connector", None),
            ProbeSubject::Runtime => ("runtime", None),
        };
        let details = &result.details;
        Self {
            subject: subject.to_owned(),
            model_id,
            status: result.status.as_str().to_owned(),
            message: result.message().to_owned(),
            note: result.note.map(|note| match note {
                tandem_core::config::ProbeNote::NoResults => "no_results".to_owned(),
            }),
            details: ProbeDetailsDto {
                tool_calling: details.tool_calling.map(|value| value.as_str().to_owned()),
                context_tokens: details.context_tokens,
                account_login: details.account_login.clone(),
                token_expires_at: details.token_expires_at.map(timestamp),
                repositories: details.repositories.clone(),
                repository_total: details.repository_total,
                runtime_name: details.runtime_name.clone(),
                runtime_version: details.runtime_version.clone(),
            },
        }
    }
}

/// Why a run failed as a whole.
#[derive(Debug, Serialize, ToSchema)]
pub struct FailureDto {
    /// `could_not_complete` or `no_worker_available`.
    pub code: String,
    /// A fixed sentence.
    pub message: String,
}

impl From<FailureReason> for FailureDto {
    fn from(reason: FailureReason) -> Self {
        Self {
            code: reason.as_str().to_owned(),
            message: reason.message().to_owned(),
        }
    }
}

/// A test run: queued, running or finished, with the results so far.
#[derive(Debug, Serialize, ToSchema)]
pub struct TestRunDto {
    /// The run's ID.
    pub id: String,
    /// What is tested: `provider:<id>`, `search:<kind>`, `connector:github` or `runtime`.
    pub target: String,
    /// `queued`, `running`, `succeeded` or `failed`.
    pub status: String,
    /// When it was started.
    pub requested_at: String,
    /// When a worker began.
    pub started_at: Option<String>,
    /// When it finished.
    pub finished_at: Option<String>,
    /// Results so far, in arrival order.
    pub results: Vec<ProbeResultDto>,
    /// Set when the run itself could not complete.
    pub failure: Option<FailureDto>,
}

impl From<&TestRun> for TestRunDto {
    fn from(run: &TestRun) -> Self {
        Self {
            id: run.id.to_string(),
            target: run.target.key(),
            status: run.status.as_str().to_owned(),
            requested_at: timestamp(run.requested_at),
            started_at: run.started_at.map(timestamp),
            finished_at: run.finished_at.map(timestamp),
            results: run.results.iter().map(ProbeResultDto::from).collect(),
            failure: run.failure.map(FailureDto::from),
        }
    }
}

// ---- providers ----------------------------------------------------------------------------

/// Something wrong with a model for a job.
#[derive(Debug, Serialize, ToSchema)]
pub struct WarningDto {
    /// `tool_calling_unsupported`, `tool_calling_unknown`, `context_too_small` or `context_unknown`.
    pub code: String,
    /// The context a model has, for `context_too_small`.
    pub have: Option<u32>,
    /// The context a job needs, for `context_too_small`.
    pub need: Option<u32>,
}

impl From<&SuitabilityWarning> for WarningDto {
    fn from(warning: &SuitabilityWarning) -> Self {
        let (code, have, need) = match warning {
            SuitabilityWarning::ToolCallingUnsupported => ("tool_calling_unsupported", None, None),
            SuitabilityWarning::ToolCallingUnknown => ("tool_calling_unknown", None, None),
            SuitabilityWarning::ContextTooSmall { have, need } => {
                ("context_too_small", Some(*have), Some(*need))
            }
            SuitabilityWarning::ContextUnknown => ("context_unknown", None, None),
        };
        Self {
            code: code.to_owned(),
            have,
            need,
        }
    }
}

/// A model with what a test found and what an admin declared.
#[derive(Debug, Serialize, ToSchema)]
pub struct ModelDto {
    /// The model's ID.
    pub id: String,
    /// The provider it belongs to.
    pub provider_id: String,
    /// The name the provider's API expects.
    pub name: String,
    /// An optional friendly label.
    pub label: Option<String>,
    /// Tool calling as used: declared if declared, otherwise detected.
    pub tool_calling: String,
    /// What a test found.
    pub detected_tool_calling: String,
    /// What an admin declared.
    pub declared_tool_calling: Option<String>,
    /// Context length as used.
    pub context_tokens: Option<u32>,
    /// What a test found.
    pub detected_context_tokens: Option<u32>,
    /// What an admin declared.
    pub declared_context_tokens: Option<u32>,
    /// Why this model may not suit a job; empty when it does.
    pub warnings: Vec<WarningDto>,
    /// A declared value disagrees with what a test found.
    pub disagrees_with_detection: bool,
}

impl From<&ModelView> for ModelDto {
    fn from(view: &ModelView) -> Self {
        let model = &view.model;
        let capabilities = &model.capabilities;
        Self {
            id: model.id.to_string(),
            provider_id: model.provider_id.to_string(),
            name: model.name.as_str().to_owned(),
            label: model.label.clone(),
            tool_calling: capabilities.tool_calling().as_str().to_owned(),
            detected_tool_calling: capabilities.detected_tool_calling.as_str().to_owned(),
            declared_tool_calling: capabilities
                .declared_tool_calling
                .map(|value| value.as_str().to_owned()),
            context_tokens: capabilities.context_tokens(),
            detected_context_tokens: capabilities.detected_context_tokens,
            declared_context_tokens: capabilities.declared_context_tokens,
            warnings: view.warnings.iter().map(WarningDto::from).collect(),
            disagrees_with_detection: view.disagrees_with_detection,
        }
    }
}

/// A provider. The key is never included.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderDto {
    /// The provider's ID.
    pub id: String,
    /// `openai_compatible`.
    pub kind: String,
    /// Display name.
    pub name: String,
    /// Base URL.
    pub base_url: String,
    /// Whether an API key is saved.
    pub api_key_saved: bool,
    /// When the key was last saved.
    pub api_key_saved_at: Option<String>,
    /// Created.
    pub created_at: String,
    /// Last changed.
    pub updated_at: String,
    /// Listed models.
    pub models: Vec<ModelDto>,
    /// The newest finished test.
    pub last_test: Option<TestRunDto>,
    /// A test queued or running now.
    pub active_test_id: Option<String>,
}

impl From<&ProviderView> for ProviderDto {
    fn from(view: &ProviderView) -> Self {
        let provider = &view.provider;
        Self {
            id: provider.id.to_string(),
            kind: provider.kind.as_str().to_owned(),
            name: provider.name.as_str().to_owned(),
            base_url: provider.base_url.as_str().to_owned(),
            api_key_saved: provider.key_saved_at.is_some(),
            api_key_saved_at: provider.key_saved_at.map(timestamp),
            created_at: timestamp(provider.created_at),
            updated_at: timestamp(provider.updated_at),
            models: view.models.iter().map(ModelDto::from).collect(),
            last_test: view.last_test.as_ref().map(TestRunDto::from),
            active_test_id: view.active_test.map(|id| id.to_string()),
        }
    }
}

/// A model as entered.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ModelRequest {
    /// The existing model this edits; leave out for a new one.
    pub id: Option<String>,
    /// The name the provider's API expects.
    pub name: String,
    /// An optional friendly label.
    #[serde(default)]
    pub label: String,
    /// Declare tool calling: `supported`, `unsupported` or `unknown`.
    pub declared_tool_calling: Option<String>,
    /// Declare the context length in tokens.
    pub declared_context_tokens: Option<u32>,
}

/// A provider as entered.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ProviderRequest {
    /// Display name.
    pub name: String,
    /// Base URL, including any `/v1`.
    pub base_url: String,
    /// A new API key. Absent or empty keeps the saved one.
    pub api_key: Option<String>,
    /// Delete the saved key.
    #[serde(default)]
    pub remove_api_key: bool,
    /// The listed models, in order.
    #[serde(default)]
    pub models: Vec<ModelRequest>,
}

fn tool_calling(text: &str) -> Result<ToolCalling, AppError> {
    ToolCalling::parse(text).ok_or_else(|| {
        AppError::validation(
            "models",
            "Tool calling must be `supported`, `unsupported` or `unknown`.",
        )
    })
}

impl ProviderRequest {
    /// Convert to the application's input.
    ///
    /// # Errors
    /// A validation error naming the field.
    pub fn into_input(self) -> Result<ProviderInput, ApiError> {
        let mut models = Vec::with_capacity(self.models.len());
        for model in self.models {
            models.push(ModelInput {
                id: model
                    .id
                    .as_deref()
                    .map(|text| {
                        text.parse::<ProviderModelId>()
                            .map_err(|_| AppError::validation("models", "That is not a model ID."))
                    })
                    .transpose()?,
                name: model.name,
                label: model.label,
                declared_tool_calling: model
                    .declared_tool_calling
                    .as_deref()
                    .map(tool_calling)
                    .transpose()?,
                declared_context_tokens: model.declared_context_tokens,
            });
        }
        Ok(ProviderInput {
            name: self.name,
            base_url: self.base_url,
            key: SecretInput::from_form(self.api_key.as_deref(), self.remove_api_key),
            models,
        })
    }
}

/// What removing a provider does.
#[derive(Debug, Serialize, ToSchema)]
pub struct RemovalImpactDto {
    /// The provider's name.
    pub provider_name: String,
    /// How many models go with it.
    pub model_count: usize,
    /// Defaults that are or will be cleared: `orchestrator` and/or `agents`.
    pub cleared_defaults: Vec<String>,
}

fn role_name(role: DefaultRole) -> &'static str {
    match role {
        DefaultRole::Orchestrator => "orchestrator",
        DefaultRole::Agents => "agents",
    }
}

impl From<&RemovalImpact> for RemovalImpactDto {
    fn from(impact: &RemovalImpact) -> Self {
        Self {
            provider_name: impact.provider_name.clone(),
            model_count: impact.model_count,
            cleared_defaults: impact
                .cleared_defaults
                .iter()
                .map(|role| role_name(*role).to_owned())
                .collect(),
        }
    }
}

// ---- defaults -----------------------------------------------------------------------------

/// One default model and where it comes from.
#[derive(Debug, Serialize, ToSchema)]
pub struct DefaultChoiceDto {
    /// The provider's name.
    pub provider_name: String,
    /// The model.
    pub model: ModelDto,
}

impl From<&DefaultChoice> for DefaultChoiceDto {
    fn from(choice: &DefaultChoice) -> Self {
        let view = ModelView {
            warnings: choice.warnings.clone(),
            disagrees_with_detection: choice.model.capabilities.disagrees_with_detection(),
            model: choice.model.clone(),
        };
        Self {
            provider_name: choice.provider_name.clone(),
            model: ModelDto::from(&view),
        }
    }
}

/// The two default models. A default with `warnings` is allowed but flagged.
#[derive(Debug, Serialize, ToSchema)]
pub struct DefaultsDto {
    /// The orchestrator's model.
    pub orchestrator: Option<DefaultChoiceDto>,
    /// The model for thread and task agents.
    pub agents: Option<DefaultChoiceDto>,
}

impl From<&DefaultsView> for DefaultsDto {
    fn from(view: &DefaultsView) -> Self {
        Self {
            orchestrator: view.orchestrator.as_ref().map(DefaultChoiceDto::from),
            agents: view.agents.as_ref().map(DefaultChoiceDto::from),
        }
    }
}

/// The picks, by model ID; leave one out to unset it.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DefaultsRequest {
    /// The orchestrator's model.
    pub orchestrator_model_id: Option<String>,
    /// The model for thread and task agents.
    pub agents_model_id: Option<String>,
}

impl DefaultsRequest {
    /// Convert to the application's input.
    ///
    /// # Errors
    /// A validation error naming the field.
    pub fn into_input(self) -> Result<tandem_app::DefaultsInput, ApiError> {
        let parse = |field: &'static str, text: Option<String>| {
            text.map(|text| {
                text.parse::<ProviderModelId>()
                    .map_err(|_| AppError::validation(field, "That is not a model ID."))
            })
            .transpose()
        };
        Ok(tandem_app::DefaultsInput {
            orchestrator: parse("orchestrator_model_id", self.orchestrator_model_id)?,
            agents: parse("agents_model_id", self.agents_model_id)?,
        })
    }
}

// ---- search -------------------------------------------------------------------------------

/// One search provider's settings and test state. The key is never included.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchKindDto {
    /// `searxng`, `brave` or `tavily`.
    pub kind: String,
    /// Settings have been saved.
    pub configured: bool,
    /// This is the active provider.
    pub active: bool,
    /// The admin supplies the base URL (SearXNG).
    pub needs_url: bool,
    /// An API key is needed (Brave, Tavily).
    pub needs_key: bool,
    /// SearXNG's base URL.
    pub base_url: Option<String>,
    /// Whether an API key is saved.
    pub api_key_saved: bool,
    /// When the key was last saved.
    pub api_key_saved_at: Option<String>,
    /// The newest finished test.
    pub last_test: Option<TestRunDto>,
    /// A test queued or running now.
    pub active_test_id: Option<String>,
}

fn search_kind_dto(view: &SearchView, active: Option<SearchKind>) -> SearchKindDto {
    SearchKindDto {
        kind: view.kind.as_str().to_owned(),
        configured: view.configured,
        active: active == Some(view.kind),
        needs_url: view.kind.needs_url(),
        needs_key: view.kind.needs_key(),
        base_url: view.base_url.as_ref().map(|url| url.as_str().to_owned()),
        api_key_saved: view.key_saved_at.is_some(),
        api_key_saved_at: view.key_saved_at.map(timestamp),
        last_test: view.last_test.as_ref().map(TestRunDto::from),
        active_test_id: view.active_test.map(|id| id.to_string()),
    }
}

/// All search providers.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchOverviewDto {
    /// One entry per kind.
    pub kinds: Vec<SearchKindDto>,
    /// The active kind, if any.
    pub active: Option<String>,
}

impl From<&SearchOverview> for SearchOverviewDto {
    fn from(overview: &SearchOverview) -> Self {
        Self {
            kinds: overview
                .kinds
                .iter()
                .map(|view| search_kind_dto(view, overview.active))
                .collect(),
            active: overview.active.map(|kind| kind.as_str().to_owned()),
        }
    }
}

impl SearchKindDto {
    /// From one saved view; `active` says whether it is the active kind.
    pub fn of(view: &SearchView, active: bool) -> Self {
        search_kind_dto(view, active.then_some(view.kind))
    }
}

/// Search settings as entered.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    /// SearXNG's base URL; empty uses the bundled service.
    pub base_url: Option<String>,
    /// A new API key. Absent or empty keeps the saved one.
    pub api_key: Option<String>,
    /// Delete the saved key.
    #[serde(default)]
    pub remove_api_key: bool,
}

impl SearchRequest {
    /// Convert to the application's input.
    pub fn into_input(self, kind: SearchKind) -> SearchInput {
        SearchInput {
            kind,
            base_url: self.base_url,
            key: SecretInput::from_form(self.api_key.as_deref(), self.remove_api_key),
        }
    }
}

/// Which search provider to turn on; leave out to turn search off.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ActiveSearchRequest {
    /// `searxng`, `brave` or `tavily`.
    pub kind: Option<String>,
}

/// The result of turning search on or off.
#[derive(Debug, Serialize, ToSchema)]
pub struct ActivationDto {
    /// The active kind afterwards.
    pub active: Option<String>,
    /// The provider has never passed a test.
    pub untested: bool,
}

impl From<SearchActivation> for ActivationDto {
    fn from(activation: SearchActivation) -> Self {
        Self {
            active: activation.active.map(|kind| kind.as_str().to_owned()),
            untested: activation.untested,
        }
    }
}

// ---- connector ----------------------------------------------------------------------------

/// The GitHub connector. The token is never included.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConnectorDto {
    /// A token is saved.
    pub configured: bool,
    /// When the token was saved.
    pub saved_at: Option<String>,
    /// The account the token belongs to, after a successful test.
    pub account_login: Option<String>,
    /// When the token expires, when GitHub says.
    pub token_expires_at: Option<String>,
    /// The newest finished test.
    pub last_test: Option<TestRunDto>,
    /// A test queued or running now.
    pub active_test_id: Option<String>,
}

impl ConnectorDto {
    /// No token saved.
    pub fn not_configured() -> Self {
        Self {
            configured: false,
            saved_at: None,
            account_login: None,
            token_expires_at: None,
            last_test: None,
            active_test_id: None,
        }
    }
}

impl From<&ConnectorView> for ConnectorDto {
    fn from(view: &ConnectorView) -> Self {
        Self {
            configured: true,
            saved_at: Some(timestamp(view.connector.secret_saved_at)),
            account_login: view.connector.account_login.clone(),
            token_expires_at: view.connector.token_expires_at.map(timestamp),
            last_test: view.last_test.as_ref().map(TestRunDto::from),
            active_test_id: view.active_test.map(|id| id.to_string()),
        }
    }
}

/// The token as entered.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ConnectorRequest {
    /// A fine-grained personal access token.
    pub token: String,
}

// ---- sandbox policy -----------------------------------------------------------------------

/// The network rule.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkDto {
    /// `none` or `allow_hosts`.
    pub mode: String,
    /// The allowed host names (only for `allow_hosts`).
    #[serde(default)]
    pub hosts: Vec<String>,
}

/// Resource limits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct LimitsDto {
    /// Wall-clock seconds per run.
    pub wall_clock_seconds: u64,
    /// Output bytes per run.
    pub max_output_bytes: u64,
    /// Largest single file a run may write.
    pub max_file_bytes: u64,
    /// Files a run may create or change.
    pub max_files_written: u64,
    /// Memory in MiB (enforced by the container runner only).
    pub memory_mebibytes: u64,
    /// CPU in millicores (enforced by the container runner only).
    pub cpu_millicores: u64,
}

impl From<Limits> for LimitsDto {
    fn from(limits: Limits) -> Self {
        Self {
            wall_clock_seconds: limits.wall_clock_seconds,
            max_output_bytes: limits.max_output_bytes,
            max_file_bytes: limits.max_file_bytes,
            max_files_written: limits.max_files_written,
            memory_mebibytes: limits.memory_mebibytes,
            cpu_millicores: limits.cpu_millicores,
        }
    }
}

/// The sandbox policy.
#[derive(Debug, Serialize, ToSchema)]
pub struct PolicyDto {
    /// `in_process` or `container`.
    pub runner_tier: String,
    /// Allowed tools.
    pub tool_allow_list: Vec<String>,
    /// The network rule.
    pub network: NetworkDto,
    /// Limits.
    pub limits: LimitsDto,
    /// Rises with every saved change.
    pub version: u32,
}

impl From<&SandboxPolicy> for PolicyDto {
    fn from(policy: &SandboxPolicy) -> Self {
        let network = match &policy.network {
            NetworkRule::None => NetworkDto {
                mode: "none".to_owned(),
                hosts: Vec::new(),
            },
            NetworkRule::AllowHosts(hosts) => NetworkDto {
                mode: "allow_hosts".to_owned(),
                hosts: hosts
                    .iter()
                    .map(|host: &HostName| host.as_str().to_owned())
                    .collect(),
            },
        };
        Self {
            runner_tier: policy.runner_tier.as_str().to_owned(),
            tool_allow_list: policy
                .tool_allow_list
                .iter()
                .map(|tool| tool.as_str().to_owned())
                .collect(),
            network,
            limits: policy.limits.into(),
            version: policy.version,
        }
    }
}

/// An allowed range for one limit.
#[derive(Debug, Serialize, ToSchema)]
pub struct RangeDto {
    /// The limit's field name.
    pub field: String,
    /// Smallest allowed.
    pub min: u64,
    /// Largest allowed.
    pub max: u64,
    /// The factory default.
    pub default: u64,
}

/// A tool in the registry.
#[derive(Debug, Serialize, ToSchema)]
pub struct ToolDto {
    /// The tool's identifier.
    pub id: String,
    /// What it does.
    pub description: String,
    /// `false` for `command_run`, which is listed but never enabled.
    pub can_be_enabled: bool,
}

/// The policy with what the editor needs: factory values, ranges and the tool registry.
#[derive(Debug, Serialize, ToSchema)]
pub struct PolicyViewDto {
    /// The policy in force.
    pub policy: PolicyDto,
    /// The factory default, for "Reset to default".
    pub factory: PolicyDto,
    /// Allowed ranges.
    pub ranges: Vec<RangeDto>,
    /// Every registered tool.
    pub tools: Vec<ToolDto>,
}

impl PolicyViewDto {
    /// Describe `policy` together with the registry and the factory values.
    pub fn of(policy: &SandboxPolicy) -> Self {
        let range = |field: &str, range: sandbox_policy::Range| RangeDto {
            field: field.to_owned(),
            min: range.min,
            max: range.max,
            default: range.default,
        };
        Self {
            policy: PolicyDto::from(policy),
            factory: PolicyDto::from(&SandboxPolicy::factory_default()),
            ranges: vec![
                range("wall_clock_seconds", sandbox_policy::WALL_CLOCK_SECONDS),
                range("max_output_bytes", sandbox_policy::MAX_OUTPUT_BYTES),
                range("max_file_bytes", sandbox_policy::MAX_FILE_BYTES),
                range("max_files_written", sandbox_policy::MAX_FILES_WRITTEN),
                range("memory_mebibytes", sandbox_policy::MEMORY_MEBIBYTES),
                range("cpu_millicores", sandbox_policy::CPU_MILLICORES),
            ],
            tools: ToolId::ALL
                .iter()
                .map(|tool| ToolDto {
                    id: tool.as_str().to_owned(),
                    description: tool.description().to_owned(),
                    can_be_enabled: tool.can_be_enabled(),
                })
                .collect(),
        }
    }
}

/// The policy as entered. The version and scope are not accepted.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PolicyRequest {
    /// `in_process` or `container`.
    pub runner_tier: String,
    /// Allowed tools.
    pub tool_allow_list: Vec<String>,
    /// The network rule.
    pub network: NetworkDto,
    /// Limits.
    pub limits: LimitsDto,
}

impl PolicyRequest {
    /// Convert to a candidate policy; every problem found is reported.
    ///
    /// # Errors
    /// A validation error naming each field.
    pub fn into_candidate(self) -> Result<SandboxPolicy, ApiError> {
        let mut issues: Vec<FieldIssue> = Vec::new();
        let tier = RunnerTier::parse(&self.runner_tier);
        if tier.is_none() {
            issues.push(FieldIssue {
                field: "runner_tier",
                message: "The runner tier must be `in_process` or `container`.".to_owned(),
            });
        }
        let mut tools = std::collections::BTreeSet::new();
        for text in &self.tool_allow_list {
            match ToolId::parse(text) {
                Some(tool) => {
                    tools.insert(tool);
                }
                None => issues.push(FieldIssue {
                    field: "tool_allow_list",
                    message: "That tool does not exist.".to_owned(),
                }),
            }
        }
        let network = match self.network.mode.as_str() {
            "none" => Some(NetworkRule::None),
            "allow_hosts" => match NetworkRule::allow_hosts(&self.network.hosts) {
                Ok(rule) => Some(rule),
                Err(found) => {
                    issues.extend(found.iter().map(|issue| FieldIssue {
                        field: issue.field(),
                        message: issue.to_string(),
                    }));
                    None
                }
            },
            _ => {
                issues.push(FieldIssue {
                    field: "network",
                    message: "The network mode must be `none` or `allow_hosts`.".to_owned(),
                });
                None
            }
        };
        match (tier, network) {
            (Some(runner_tier), Some(network)) if issues.is_empty() => Ok(SandboxPolicy {
                scope: PolicyScope::Instance,
                runner_tier,
                tool_allow_list: tools,
                network,
                limits: Limits {
                    wall_clock_seconds: self.limits.wall_clock_seconds,
                    max_output_bytes: self.limits.max_output_bytes,
                    max_file_bytes: self.limits.max_file_bytes,
                    max_files_written: self.limits.max_files_written,
                    memory_mebibytes: self.limits.memory_mebibytes,
                    cpu_millicores: self.limits.cpu_millicores,
                },
                version: 0,
            }),
            _ => Err(AppError::Validation(issues).into()),
        }
    }
}
