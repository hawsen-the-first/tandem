use super::*;
use crate::ids::{ProviderId, ProviderModelId, TestRunId, UserId};
use crate::outbound_url::OutboundUrl;
use tandem_support::{FakeClock, FakeRng};
use time::{Duration, OffsetDateTime};

fn clock() -> FakeClock {
    FakeClock::at_unix(1_800_000_000)
}

fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
}

fn model_id(seed: u64) -> ProviderModelId {
    ProviderModelId::generate(&clock(), &FakeRng::new(seed)).unwrap()
}

// ---- names and lists (BR-14.2, BR-14.3) -------------------------------------------------------

#[test]
fn br_14_2_provider_names_are_trimmed_limited_and_compared_ignoring_case() {
    assert_eq!(
        ProviderName::parse("  Local Ollama ").unwrap().as_str(),
        "Local Ollama"
    );
    assert_eq!(
        ProviderName::parse("Local OLLAMA").unwrap().unique_key(),
        ProviderName::parse("local ollama").unwrap().unique_key()
    );
    assert_eq!(
        ProviderName::parse("").unwrap_err(),
        ConfigRuleError::Empty("The name")
    );
    assert_eq!(
        ProviderName::parse("   ").unwrap_err(),
        ConfigRuleError::Empty("The name")
    );
    assert!(ProviderName::parse(&"a".repeat(60)).is_ok());
    assert!(matches!(
        ProviderName::parse(&"a".repeat(61)).unwrap_err(),
        ConfigRuleError::TooLong { max: 60, .. }
    ));
    assert!(matches!(
        ProviderName::parse("bad\nname").unwrap_err(),
        ConfigRuleError::ControlCharacters(_)
    ));
    // Characters are counted, not bytes.
    assert!(ProviderName::parse(&"é".repeat(60)).is_ok());
}

#[test]
fn br_14_3_model_names_and_lists_are_validated() {
    assert_eq!(
        ModelName::parse(" llama3.1:8b ").unwrap().as_str(),
        "llama3.1:8b"
    );
    assert!(ModelName::parse("").is_err());
    assert!(ModelName::parse(&"m".repeat(201)).is_err());
    assert!(ModelName::parse(&"m".repeat(200)).is_ok());

    let names: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|n| ModelName::parse(n).unwrap())
        .collect();
    assert!(validate_model_names(&names).is_ok());
    assert!(
        validate_model_names(&[]).is_ok(),
        "a provider may list no models"
    );
    let duplicated: Vec<_> = ["a", "b", "a"]
        .iter()
        .map(|n| ModelName::parse(n).unwrap())
        .collect();
    assert_eq!(
        validate_model_names(&duplicated).unwrap_err(),
        ConfigRuleError::DuplicateModel
    );
    let too_many: Vec<_> = (0..=MAX_MODELS_PER_PROVIDER)
        .map(|n| ModelName::parse(&format!("m{n}")).unwrap())
        .collect();
    assert!(matches!(
        validate_model_names(&too_many).unwrap_err(),
        ConfigRuleError::TooMany { .. }
    ));
}

#[test]
fn model_labels_are_optional_and_limited() {
    assert_eq!(ProviderModel::clean_label("  ").unwrap(), None);
    assert_eq!(
        ProviderModel::clean_label(" Fast ").unwrap().as_deref(),
        Some("Fast")
    );
    assert!(ProviderModel::clean_label(&"x".repeat(81)).is_err());
}

#[test]
fn provider_kinds_and_tool_calling_round_trip() {
    assert_eq!(
        ProviderKind::parse("openai_compatible"),
        Some(ProviderKind::OpenAiCompatible)
    );
    assert_eq!(ProviderKind::parse("anthropic"), None);
    for value in [
        ToolCalling::Supported,
        ToolCalling::Unsupported,
        ToolCalling::Unknown,
    ] {
        assert_eq!(ToolCalling::parse(value.as_str()), Some(value));
    }
}

// ---- capabilities (BR-14.5, BR-14.8, BR-15.2) ---------------------------------------------------

#[test]
fn br_14_5_a_test_replaces_detected_values_and_never_a_declared_one() {
    let mut capabilities = ModelCapabilities::default();
    capabilities.declare_context_tokens(32_000).unwrap();
    capabilities.declared_tool_calling = Some(ToolCalling::Supported);

    capabilities.apply_detection(ToolCalling::Unsupported, Some(8_192));
    assert_eq!(capabilities.detected_tool_calling, ToolCalling::Unsupported);
    assert_eq!(capabilities.detected_context_tokens, Some(8_192));
    assert_eq!(
        capabilities.tool_calling(),
        ToolCalling::Supported,
        "declared wins"
    );
    assert_eq!(capabilities.context_tokens(), Some(32_000), "declared wins");
    assert!(capabilities.disagrees_with_detection());

    // A later test that learns nothing about context keeps the old detected context.
    capabilities.apply_detection(ToolCalling::Supported, None);
    assert_eq!(capabilities.detected_context_tokens, Some(8_192));
    assert_eq!(capabilities.detected_tool_calling, ToolCalling::Supported);
}

#[test]
fn undeclared_values_follow_detection_and_agreement_is_not_a_disagreement() {
    let mut capabilities = ModelCapabilities::default();
    assert_eq!(capabilities.tool_calling(), ToolCalling::Unknown);
    assert_eq!(capabilities.context_tokens(), None);
    assert!(!capabilities.disagrees_with_detection());
    capabilities.apply_detection(ToolCalling::Supported, Some(128_000));
    assert_eq!(capabilities.tool_calling(), ToolCalling::Supported);
    assert_eq!(capabilities.context_tokens(), Some(128_000));
    capabilities.declared_tool_calling = Some(ToolCalling::Supported);
    capabilities.declare_context_tokens(128_000).unwrap();
    assert!(!capabilities.disagrees_with_detection());
    let unknown = ModelCapabilities {
        declared_tool_calling: Some(ToolCalling::Supported),
        ..ModelCapabilities::default()
    };
    assert!(
        !unknown.disagrees_with_detection(),
        "nothing detected, nothing to disagree with"
    );
}

#[test]
fn declared_context_must_be_in_range() {
    let mut capabilities = ModelCapabilities::default();
    assert!(capabilities.declare_context_tokens(0).is_err());
    assert!(
        capabilities
            .declare_context_tokens(MAX_CONTEXT_TOKENS + 1)
            .is_err()
    );
    assert!(capabilities.declare_context_tokens(1).is_ok());
    assert!(
        capabilities
            .declare_context_tokens(MAX_CONTEXT_TOKENS)
            .is_ok()
    );
}

#[test]
fn br_15_2_suitability_needs_tool_calling_and_enough_context() {
    let make = |tool_calling, context| {
        let mut capabilities = ModelCapabilities::default();
        capabilities.apply_detection(tool_calling, context);
        capabilities
    };
    assert!(
        make(ToolCalling::Supported, Some(8_000))
            .suitability_warnings()
            .is_empty()
    );
    assert!(
        make(ToolCalling::Supported, Some(128_000))
            .suitability_warnings()
            .is_empty()
    );
    assert_eq!(
        make(ToolCalling::Supported, Some(7_999)).suitability_warnings(),
        vec![SuitabilityWarning::ContextTooSmall {
            have: 7_999,
            need: 8_000
        }]
    );
    assert_eq!(
        make(ToolCalling::Unsupported, Some(32_000)).suitability_warnings(),
        vec![SuitabilityWarning::ToolCallingUnsupported]
    );
    assert_eq!(
        make(ToolCalling::Unknown, None).suitability_warnings(),
        vec![
            SuitabilityWarning::ToolCallingUnknown,
            SuitabilityWarning::ContextUnknown
        ]
    );
    assert_eq!(
        make(ToolCalling::Supported, None).suitability_warnings(),
        vec![SuitabilityWarning::ContextUnknown]
    );
}

// ---- defaults (BR-15) ---------------------------------------------------------------------------

#[test]
fn br_15_3_removing_models_clears_the_defaults_that_pointed_at_them() {
    let (first, second, third) = (model_id(1), model_id(2), model_id(3));
    let mut defaults = ModelDefaults {
        orchestrator: Some(first),
        agents: Some(second),
    };
    assert_eq!(defaults.get(DefaultRole::Orchestrator), Some(first));
    assert_eq!(defaults.clear_models(&[third]), vec![]);
    assert_eq!(
        defaults.clear_models(&[second, third]),
        vec![DefaultRole::Agents]
    );
    assert_eq!(defaults.agents, None);
    assert_eq!(defaults.orchestrator, Some(first));
    assert_eq!(
        defaults.clear_models(&[first]),
        vec![DefaultRole::Orchestrator]
    );
    assert_eq!(ModelDefaults::default().clear_models(&[first]), vec![]);
}

// ---- search and connectors (BR-16, BR-17) -------------------------------------------------------

#[test]
fn br_16_search_kinds_know_what_they_need() {
    assert!(SearchKind::Searxng.needs_url() && !SearchKind::Searxng.needs_key());
    for kind in [SearchKind::Brave, SearchKind::Tavily] {
        assert!(!kind.needs_url() && kind.needs_key());
    }
    for kind in SearchKind::ALL {
        assert_eq!(SearchKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(SearchKind::parse("google"), None);
    assert!(
        OutboundUrl::parse(DEFAULT_SEARXNG_URL).is_ok(),
        "the default address passes BR-19"
    );
    assert_eq!(ConnectorKind::parse("github"), Some(ConnectorKind::Github));
    assert_eq!(ConnectorKind::parse("gitlab"), None);
    assert_eq!(ConnectorAuth::parse("token"), Some(ConnectorAuth::Token));
    assert_eq!(
        ConnectorAuth::parse("app"),
        None,
        "the GitHub App arrives in U5"
    );
}

// ---- test results (BR-14.6, SE-22) --------------------------------------------------------------

#[test]
fn test_statuses_round_trip_and_have_fixed_messages() {
    let all = [
        TestStatus::Ok,
        TestStatus::Unreachable,
        TestStatus::Timeout,
        TestStatus::Unauthorised,
        TestStatus::UnknownModel,
        TestStatus::BadResponse,
        TestStatus::RefusedAddress,
        TestStatus::NotConfigured,
    ];
    let mut messages = std::collections::HashSet::new();
    for status in all {
        assert_eq!(TestStatus::parse(status.as_str()), Some(status));
        assert!(
            messages.insert(status.message()),
            "each status has its own message"
        );
        assert!(status.message().ends_with('.'));
    }
    assert_eq!(TestStatus::parse("success"), None);
    assert_eq!(
        ProbeResult::new(ProbeSubject::Provider, TestStatus::Timeout).message(),
        TestStatus::Timeout.message()
    );
    assert!(ProbeNote::NoResults.message().contains("no results"));
}

#[test]
fn targets_and_subjects_round_trip_through_their_keys() {
    let provider = ProviderId::generate(&clock(), &FakeRng::new(9)).unwrap();
    for target in [
        TestTarget::Provider(provider),
        TestTarget::Search(SearchKind::Brave),
        TestTarget::Connector(ConnectorKind::Github),
        TestTarget::Runtime,
    ] {
        assert_eq!(TestTarget::parse_key(&target.key()), Some(target));
    }
    assert_eq!(TestTarget::parse_key("provider:not-a-uuid"), None);
    assert_eq!(TestTarget::parse_key("search:google"), None);
    assert_eq!(TestTarget::parse_key("nonsense"), None);
    assert_eq!(TestTarget::Provider(provider).kind_name(), "provider");
    assert_eq!(
        TestTarget::Connector(ConnectorKind::Github).kind_name(),
        "github"
    );
    for subject in [
        ProbeSubject::Provider,
        ProbeSubject::Model(model_id(4)),
        ProbeSubject::Search,
        ProbeSubject::Connector,
        ProbeSubject::Runtime,
    ] {
        assert_eq!(ProbeSubject::parse_key(&subject.key()), Some(subject));
    }
    assert_eq!(ProbeSubject::parse_key("model:xyz"), None);
}

#[test]
fn br_17_3_only_the_first_hundred_repositories_are_kept_with_the_true_total() {
    let names: Vec<String> = (0..250).map(|n| format!("org/repo-{n}")).collect();
    let details = ProbeDetails::default().with_repositories(names, 250);
    assert_eq!(details.repositories.len(), 100);
    assert_eq!(details.repository_total, Some(250));
}

// ---- the run state machine (pattern 2, D2) -----------------------------------------------------

fn run() -> TestRun {
    TestRun::queued(
        TestRunId::generate(&clock(), &FakeRng::new(11)).unwrap(),
        TestTarget::Runtime,
        UserId::generate(&clock(), &FakeRng::new(12)).unwrap(),
        now(),
    )
}

#[test]
fn a_run_goes_queued_running_succeeded_and_keeps_its_results_in_order() {
    let mut run = run();
    assert!(run.is_active());
    assert_eq!(run.status, RunStatus::Queued);
    assert!(
        run.record(ProbeResult::new(ProbeSubject::Runtime, TestStatus::Ok))
            .is_err()
    );
    run.start(now() + Duration::seconds(1)).unwrap();
    assert_eq!(run.started_at, Some(now() + Duration::seconds(1)));
    assert!(run.is_active());
    let model = ProbeSubject::Model(model_id(5));
    run.record(ProbeResult::new(ProbeSubject::Provider, TestStatus::Ok))
        .unwrap();
    run.record(ProbeResult::new(model, TestStatus::Timeout))
        .unwrap();
    run.record(ProbeResult::new(model, TestStatus::Ok)).unwrap();
    assert_eq!(
        run.results.len(),
        2,
        "a later result for the same subject replaces the earlier one"
    );
    assert_eq!(run.results[1].status, TestStatus::Ok);
    run.finish(now() + Duration::seconds(9)).unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert!(!run.is_active());
    assert_eq!(run.finished_at, Some(now() + Duration::seconds(9)));
}

#[test]
fn transitions_out_of_order_are_refused() {
    let mut run = run();
    assert_eq!(run.finish(now()).unwrap_err().from, RunStatus::Queued);
    assert!(run.requeue().is_err());
    run.start(now()).unwrap();
    assert!(run.start(now()).is_err(), "cannot start twice");
    run.finish(now()).unwrap();
    assert!(
        run.fail(now(), FailureReason::CouldNotComplete).is_err(),
        "finished runs stay finished"
    );
    assert!(
        run.record(ProbeResult::new(ProbeSubject::Runtime, TestStatus::Ok))
            .is_err()
    );
}

#[test]
fn d2_a_retried_job_restarts_the_run_with_no_partial_results() {
    let mut run = run();
    run.start(now()).unwrap();
    run.record(ProbeResult::new(ProbeSubject::Provider, TestStatus::Ok))
        .unwrap();
    run.requeue().unwrap();
    assert_eq!(run.status, RunStatus::Queued);
    assert!(run.results.is_empty());
    assert_eq!(run.started_at, None);
    assert!(run.is_active());
    run.start(now()).unwrap();
}

#[test]
fn d2_a_run_that_cannot_complete_fails_with_a_fixed_reason() {
    let mut queued = run();
    queued
        .fail(now(), FailureReason::NoWorkerAvailable)
        .unwrap();
    assert_eq!(queued.status, RunStatus::Failed);
    assert_eq!(queued.failure, Some(FailureReason::NoWorkerAvailable));
    assert!(!queued.is_active());

    let mut running = run();
    running.start(now()).unwrap();
    running
        .fail(now(), FailureReason::CouldNotComplete)
        .unwrap();
    assert_eq!(
        running.failure.unwrap().message(),
        "The test could not complete."
    );
    for reason in [
        FailureReason::CouldNotComplete,
        FailureReason::NoWorkerAvailable,
    ] {
        assert_eq!(FailureReason::parse(reason.as_str()), Some(reason));
    }
    for status in [
        RunStatus::Queued,
        RunStatus::Running,
        RunStatus::Succeeded,
        RunStatus::Failed,
    ] {
        assert_eq!(RunStatus::parse(status.as_str()), Some(status));
    }
}
