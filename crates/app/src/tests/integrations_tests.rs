use super::*;
use crate::{DefaultsInput, ModelInput, ProviderInput, ProviderView, SearchInput, SecretInput};
use tandem_core::config::{DefaultRole, SearchKind, SuitabilityWarning, ToolCalling};
use tandem_core::sandbox_policy::{NetworkRule, RunnerTier, SandboxPolicy, ToolId};
use tandem_support::Secret;

const KEY: &str = "sk-test-do-not-store-or-print";

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    (t, admin)
}

fn model(name: &str) -> ModelInput {
    ModelInput {
        id: None,
        name: name.to_owned(),
        label: String::new(),
        declared_tool_calling: None,
        declared_context_tokens: None,
    }
}

fn input(name: &str, models: &[&str]) -> ProviderInput {
    ProviderInput {
        name: name.to_owned(),
        base_url: "http://localhost:11434/v1".to_owned(),
        key: SecretInput::Set(Secret::new(KEY)),
        models: models.iter().map(|name| model(name)).collect(),
    }
}

async fn create(t: &TestApp, admin: &RequestContext, name: &str, models: &[&str]) -> ProviderView {
    t.app
        .create_provider(admin, input(name, models))
        .await
        .unwrap()
}

fn audit_details(t: &TestApp, event_type: &str) -> Vec<std::collections::BTreeMap<String, String>> {
    t.store.read(|s| {
        s.audit
            .iter()
            .filter(|entry| entry.event.event_type.as_str() == event_type)
            .map(|entry| entry.event.details.clone())
            .collect()
    })
}

#[tokio::test]
async fn br_14_2_a_provider_is_created_with_its_models_and_a_sealed_key() {
    let (t, admin) = setup().await;
    let view = create(&t, &admin, "Local Ollama", &["llama3", "qwen"]).await;
    assert_eq!(view.provider.name.as_str(), "Local Ollama");
    assert_eq!(view.models.len(), 2);
    assert!(view.provider.key_saved_at.is_some());
    let sealed = t
        .store
        .read(|s| s.config.providers[0].sealed_key.clone())
        .unwrap();
    let expected_prefix = format!("sealed:provider_api_key:{}:", view.provider.id);
    assert!(
        sealed.starts_with(expected_prefix.as_bytes()),
        "BR-20.1: sealed under its purpose, bound to its row"
    );
    assert_eq!(count_audit(&t, "config.provider.created"), 1);
}

#[tokio::test]
async fn br_20_2_and_20_4_no_view_or_audit_entry_carries_the_key() {
    let (t, admin) = setup().await;
    let view = create(&t, &admin, "Local", &["m"]).await;
    let listed = t.app.list_providers(&admin).await.unwrap();
    let fetched = t.app.get_provider(&admin, view.provider.id).await.unwrap();
    assert!(!format!("{view:?}{listed:?}{fetched:?}").contains(KEY));
    let audit = t.store.read(|s| format!("{:?}", s.audit));
    assert!(!audit.contains(KEY));
    assert_eq!(
        audit_details(&t, "config.provider.created")[0]["api_key"],
        "set"
    );
}

#[tokio::test]
async fn br_14_2_names_are_unique_ignoring_case_and_every_field_problem_is_reported() {
    let (t, admin) = setup().await;
    create(&t, &admin, "Local", &[]).await;
    let duplicate = t.app.create_provider(&admin, input("LOCAL", &[])).await;
    assert!(matches!(duplicate, Err(AppError::Validation(ref i)) if i[0].field == "name"));

    let mut bad = input("  ", &["a", "a"]);
    bad.base_url = "http://169.254.169.254/latest".to_owned();
    let Err(AppError::Validation(issues)) = t.app.create_provider(&admin, bad).await else {
        panic!("expected validation errors");
    };
    let fields: Vec<_> = issues.iter().map(|issue| issue.field).collect();
    assert!(fields.contains(&"name"), "{fields:?}");
    assert!(fields.contains(&"base_url"), "{fields:?}");
    assert!(fields.contains(&"models"), "{fields:?}");
    let url_issue = issues
        .iter()
        .find(|issue| issue.field == "base_url")
        .unwrap();
    assert!(
        url_issue.message.contains("not allowed"),
        "BR-19.7: {}",
        url_issue.message
    );
}

#[tokio::test]
async fn sc_5_an_instance_can_have_at_most_twenty_providers() {
    let (t, admin) = setup().await;
    for number in 0..20 {
        create(&t, &admin, &format!("P{number}"), &[]).await;
    }
    let extra = t.app.create_provider(&admin, input("One more", &[])).await;
    assert!(matches!(extra, Err(AppError::Validation(ref i)) if i[0].field == "name"));
}

#[tokio::test]
async fn br_20_3_an_empty_key_keeps_the_stored_one_set_replaces_it_and_remove_clears_it() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["m"]).await;
    let id = created.provider.id;
    let first_saved = created.provider.key_saved_at;

    t.clock.advance(time::Duration::minutes(5));
    let mut edit = input("Local", &["m"]);
    edit.key = SecretInput::Keep;
    let kept = t
        .app
        .update_provider(&admin, id, edit.clone())
        .await
        .unwrap();
    assert_eq!(kept.provider.key_saved_at, first_saved);
    assert!(t.store.read(|s| s.config.providers[0].sealed_key.is_some()));

    edit.key = SecretInput::Set(Secret::new("replacement"));
    let replaced = t
        .app
        .update_provider(&admin, id, edit.clone())
        .await
        .unwrap();
    assert!(replaced.provider.key_saved_at > first_saved);
    let sealed = t
        .store
        .read(|s| s.config.providers[0].sealed_key.clone())
        .unwrap();
    assert!(sealed.ends_with(b"replacement"));

    edit.key = SecretInput::Remove;
    let removed = t.app.update_provider(&admin, id, edit).await.unwrap();
    assert_eq!(removed.provider.key_saved_at, None);
    assert!(t.store.read(|s| s.config.providers[0].sealed_key.is_none()));
    let details = audit_details(&t, "config.provider.updated");
    assert_eq!(details.len(), 3);
    assert_eq!(details[2]["api_key_before"], "set");
    assert_eq!(details[2]["api_key_after"], "not set");
}

#[tokio::test]
async fn br_14_5_editing_models_keeps_detected_capabilities_and_applies_declared_ones() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["llama3"]).await;
    let model_id = created.models[0].model.id;
    t.store.write(|s| {
        s.config.models[0]
            .capabilities
            .apply_detection(ToolCalling::Supported, Some(8192));
    });
    let mut edit = input("Local", &[]);
    edit.models = vec![ModelInput {
        id: Some(model_id),
        name: "llama3-renamed".to_owned(),
        label: "Llama".to_owned(),
        declared_tool_calling: Some(ToolCalling::Unsupported),
        declared_context_tokens: Some(4096),
    }];
    let view = t
        .app
        .update_provider(&admin, created.provider.id, edit)
        .await
        .unwrap();
    let edited = &view.models[0];
    assert_eq!(edited.model.id, model_id, "the id survives a rename");
    assert_eq!(edited.model.name.as_str(), "llama3-renamed");
    let capabilities = edited.model.capabilities;
    assert_eq!(capabilities.detected_tool_calling, ToolCalling::Supported);
    assert_eq!(
        capabilities.declared_tool_calling,
        Some(ToolCalling::Unsupported)
    );
    assert_eq!(capabilities.tool_calling(), ToolCalling::Unsupported);
    assert_eq!(capabilities.context_tokens(), Some(4096));
    assert!(edited.disagrees_with_detection);
}

#[tokio::test]
async fn br_14_8_a_model_without_what_a_job_needs_is_flagged() {
    let (t, admin) = setup().await;
    let view = create(&t, &admin, "Local", &["fresh"]).await;
    let warnings = &view.models[0].warnings;
    assert!(warnings.contains(&SuitabilityWarning::ToolCallingUnknown));
    assert!(warnings.contains(&SuitabilityWarning::ContextUnknown));
}

#[tokio::test]
async fn br_15_3_removing_a_model_clears_a_default_that_pointed_at_it() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["a", "b"]).await;
    let (a, b) = (created.models[0].model.id, created.models[1].model.id);
    t.app
        .set_defaults(
            &admin,
            DefaultsInput {
                orchestrator: Some(a),
                agents: Some(b),
            },
        )
        .await
        .unwrap();
    let mut edit = input("Local", &[]);
    edit.models = vec![ModelInput {
        id: Some(b),
        ..model("b")
    }];
    t.app
        .update_provider(&admin, created.provider.id, edit)
        .await
        .unwrap();
    let defaults = t.app.get_defaults(&admin).await.unwrap();
    assert!(
        defaults.orchestrator.is_none(),
        "removed model's default is cleared"
    );
    assert_eq!(defaults.agents.unwrap().model.id, b);
    let details = audit_details(&t, "config.provider.updated");
    assert_eq!(details[0]["cleared_defaults"], "orchestrator");
}

#[tokio::test]
async fn br_15_3_and_w18_removing_a_provider_reports_then_clears_what_depends_on_it() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["a"]).await;
    let a = created.models[0].model.id;
    t.app
        .set_defaults(
            &admin,
            DefaultsInput {
                orchestrator: Some(a),
                agents: Some(a),
            },
        )
        .await
        .unwrap();
    let impact = t
        .app
        .provider_removal_impact(&admin, created.provider.id)
        .await
        .unwrap();
    assert_eq!(impact.provider_name, "Local");
    assert_eq!(impact.model_count, 1);
    assert_eq!(
        impact.cleared_defaults,
        vec![DefaultRole::Orchestrator, DefaultRole::Agents]
    );
    assert_eq!(
        t.app
            .get_defaults(&admin)
            .await
            .unwrap()
            .orchestrator
            .unwrap()
            .model
            .id,
        a,
        "asking changes nothing"
    );

    t.app
        .delete_provider(&admin, created.provider.id)
        .await
        .unwrap();
    assert!(t.app.list_providers(&admin).await.unwrap().is_empty());
    let defaults = t.app.get_defaults(&admin).await.unwrap();
    assert!(defaults.orchestrator.is_none() && defaults.agents.is_none());
    let details = audit_details(&t, "config.provider.deleted");
    assert_eq!(details[0]["cleared_defaults"], "orchestrator,agents");
    assert!(
        t.store
            .read(|s| s.config.providers.is_empty() && s.config.models.is_empty())
    );
}

#[tokio::test]
async fn br_21_3_only_signed_in_admins_reach_any_of_it() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["a"]).await;
    let (member_ctx, _) = member(&t, &admin, "mia@example.com").await;
    let anonymous = t.anonymous();
    for ctx in [&member_ctx, &anonymous] {
        let denied = |result: Result<(), AppError>| match (ctx.actor.is_some(), result) {
            (true, Err(AppError::NotFound))
            | (false, Err(AppError::NotFound | AppError::Unauthenticated)) => {}
            (_, other) => panic!("expected refusal, got {other:?}"),
        };
        denied(t.app.list_providers(ctx).await.map(|_| ()));
        denied(
            t.app
                .create_provider(ctx, input("X", &[]))
                .await
                .map(|_| ()),
        );
        denied(
            t.app
                .delete_provider(ctx, created.provider.id)
                .await
                .map(|_| ()),
        );
        denied(t.app.get_defaults(ctx).await.map(|_| ()));
        denied(t.app.get_search(ctx).await.map(|_| ()));
        denied(t.app.get_connector(ctx).await.map(|_| ()));
        denied(t.app.get_sandbox_policy(ctx).await.map(|_| ()));
        denied(
            t.app
                .save_connector_token(ctx, Secret::new("x"))
                .await
                .map(|_| ()),
        );
    }
    assert_eq!(
        t.app.list_providers(&admin).await.unwrap().len(),
        1,
        "nothing changed"
    );
}

#[tokio::test]
async fn br_21_1_if_the_audit_write_fails_the_change_does_not_happen() {
    let (t, admin) = setup().await;
    t.store.write(|s| s.fail_audit = true);
    let result = t.app.create_provider(&admin, input("Local", &["a"])).await;
    assert!(matches!(result, Err(AppError::Internal(_))));
    assert!(
        t.store
            .read(|s| s.config.providers.is_empty() && s.config.models.is_empty())
    );
}

#[tokio::test]
async fn br_15_2_an_unsuitable_default_is_allowed_with_warnings_and_unknown_models_are_refused() {
    let (t, admin) = setup().await;
    let created = create(&t, &admin, "Local", &["a"]).await;
    let a = created.models[0].model.id;
    let saved = t
        .app
        .set_defaults(
            &admin,
            DefaultsInput {
                orchestrator: Some(a),
                agents: None,
            },
        )
        .await
        .unwrap();
    let choice = saved.orchestrator.unwrap();
    assert_eq!(choice.provider_name, "Local", "BR-15.5");
    assert!(!choice.warnings.is_empty());
    assert_eq!(
        audit_details(&t, "config.defaults.updated")[0]["orchestrator_warnings"],
        "2"
    );

    let unknown = t
        .app
        .set_defaults(
            &admin,
            DefaultsInput {
                orchestrator: Some("00000000-0000-0000-0000-0000000000ff".parse().unwrap()),
                agents: None,
            },
        )
        .await;
    assert!(matches!(unknown, Err(AppError::Validation(ref i)) if i[0].field == "orchestrator"));
}

#[tokio::test]
async fn br_16_1_one_search_provider_is_active_and_keys_are_needed_before_activation() {
    let (t, admin) = setup().await;
    let overview = t.app.get_search(&admin).await.unwrap();
    assert_eq!(overview.kinds.len(), 3);
    assert!(overview.active.is_none());

    let needs_key = t
        .app
        .set_active_search(&admin, Some(SearchKind::Brave))
        .await;
    assert!(matches!(needs_key, Err(AppError::Validation(ref i)) if i[0].field == "kind"));

    t.app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Brave,
                base_url: None,
                key: SecretInput::Set(Secret::new("brave-key")),
            },
        )
        .await
        .unwrap();
    let first = t
        .app
        .set_active_search(&admin, Some(SearchKind::Brave))
        .await
        .unwrap();
    assert!(first.untested, "BR-16.4: never tested");

    let searxng = t
        .app
        .set_active_search(&admin, Some(SearchKind::Searxng))
        .await
        .unwrap();
    assert_eq!(searxng.active, Some(SearchKind::Searxng));
    assert_eq!(
        t.app.get_search(&admin).await.unwrap().active,
        Some(SearchKind::Searxng)
    );
    let brave = t
        .app
        .get_search(&admin)
        .await
        .unwrap()
        .kinds
        .into_iter()
        .find(|v| v.kind == SearchKind::Brave)
        .unwrap();
    assert!(
        brave.key_saved_at.is_some(),
        "BR-16.3: inactive settings are kept"
    );

    t.app.set_active_search(&admin, None).await.unwrap();
    assert!(t.app.get_search(&admin).await.unwrap().active.is_none());
    assert_eq!(count_audit(&t, "config.search.updated"), 4);
}

#[tokio::test]
async fn br_16_2_searxng_defaults_to_the_bundled_service_and_other_kinds_take_no_url_or_wrong_key()
{
    let (t, admin) = setup().await;
    let view = t
        .app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Searxng,
                base_url: None,
                key: SecretInput::Keep,
            },
        )
        .await
        .unwrap();
    assert_eq!(view.base_url.unwrap().as_str(), "http://searxng:8080");

    let bad_url = t
        .app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Searxng,
                base_url: Some("ftp://x".into()),
                key: SecretInput::Keep,
            },
        )
        .await;
    assert!(matches!(bad_url, Err(AppError::Validation(ref i)) if i[0].field == "base_url"));

    let key_for_searxng = t
        .app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Searxng,
                base_url: None,
                key: SecretInput::Set(Secret::new("k")),
            },
        )
        .await;
    assert!(matches!(key_for_searxng, Err(AppError::Validation(ref i)) if i[0].field == "api_key"));
}

#[tokio::test]
async fn br_17_2_the_github_token_is_sealed_replaced_and_removed() {
    let (t, admin) = setup().await;
    assert!(t.app.get_connector(&admin).await.unwrap().is_none());
    let view = t
        .app
        .save_connector_token(&admin, Secret::new("  github_pat_abc  "))
        .await
        .unwrap();
    assert_eq!(view.connector.account_login, None);
    let sealed = t.store.read(|s| s.config.connectors[0].1.clone());
    assert!(sealed.starts_with(b"sealed:connector_secret:github:"));
    assert!(
        sealed.ends_with(b"github_pat_abc"),
        "surrounding spaces are trimmed"
    );
    assert!(!format!("{view:?}").contains("github_pat_abc"));

    t.store
        .write(|s| s.config.connectors[0].0.account_login = Some("octocat".into()));
    t.app
        .save_connector_token(&admin, Secret::new("github_pat_new"))
        .await
        .unwrap();
    assert_eq!(
        t.app
            .get_connector(&admin)
            .await
            .unwrap()
            .unwrap()
            .connector
            .account_login,
        None,
        "a replaced token forgets the old account"
    );

    let blank = t.app.save_connector_token(&admin, Secret::new("   ")).await;
    assert!(matches!(blank, Err(AppError::Validation(ref i)) if i[0].field == "token"));

    t.app.remove_connector(&admin).await.unwrap();
    assert!(t.app.get_connector(&admin).await.unwrap().is_none());
    assert!(matches!(
        t.app.remove_connector(&admin).await,
        Err(AppError::NotFound)
    ));
    let audit = t.store.read(|s| format!("{:?}", s.audit));
    assert!(!audit.contains("github_pat"));
    assert_eq!(count_audit(&t, "config.connector.updated"), 3);
}

#[tokio::test]
async fn br_18_1_an_instance_that_never_saved_a_policy_has_the_factory_default() {
    let (t, admin) = setup().await;
    let policy = t.app.get_sandbox_policy(&admin).await.unwrap();
    assert_eq!(policy, SandboxPolicy::factory_default());
    assert!(!policy.tool_allow_list.contains(&ToolId::CommandRun));
}

#[tokio::test]
async fn br_18_5_a_saved_limit_wins_and_each_save_raises_the_version() {
    let (t, admin) = setup().await;
    let mut candidate = SandboxPolicy::factory_default();
    candidate.limits.wall_clock_seconds = 1200;
    let saved = t
        .app
        .save_sandbox_policy(&admin, candidate.clone())
        .await
        .unwrap();
    assert_eq!((saved.limits.wall_clock_seconds, saved.version), (1200, 2));
    candidate.limits.max_files_written = 50;
    let again = t.app.save_sandbox_policy(&admin, candidate).await.unwrap();
    assert_eq!(again.version, 3);
    assert_eq!(t.app.get_sandbox_policy(&admin).await.unwrap(), again);
    let details = audit_details(&t, "config.sandbox_policy.updated");
    assert_eq!(details[0]["wall_clock_seconds_before"], "600");
    assert_eq!(details[0]["wall_clock_seconds_after"], "1200");
    assert_eq!(details[1]["max_files_written_after"], "50");
    assert_eq!(details[1]["version_after"], "3");
}

#[tokio::test]
async fn br_18_3_and_18_4_an_invalid_policy_reports_every_problem_by_field() {
    let (t, admin) = setup().await;
    let mut candidate = SandboxPolicy::factory_default();
    candidate.tool_allow_list.insert(ToolId::CommandRun);
    candidate.limits.wall_clock_seconds = 1;
    candidate.network = NetworkRule::AllowHosts(std::collections::BTreeSet::new());
    let Err(AppError::Validation(issues)) = t.app.save_sandbox_policy(&admin, candidate).await
    else {
        panic!("expected validation errors");
    };
    assert!(issues.len() >= 2, "{issues:?}");
    assert_eq!(
        t.app.get_sandbox_policy(&admin).await.unwrap().version,
        1,
        "nothing saved"
    );
    assert_eq!(count_audit(&t, "config.sandbox_policy.updated"), 0);
}

#[tokio::test]
async fn br_18_2_the_container_tier_needs_a_runtime_that_answers_a_fresh_probe() {
    let (t, admin) = setup().await;
    let mut candidate = SandboxPolicy::factory_default();
    candidate.runner_tier = RunnerTier::Container;

    *t.probes.runtime_status.lock().unwrap() = tandem_core::config::TestStatus::Unreachable;
    let refused = t.app.save_sandbox_policy(&admin, candidate.clone()).await;
    assert!(matches!(refused, Err(AppError::Validation(ref i)) if i[0].field == "runner_tier"));

    *t.probes.runtime_status.lock().unwrap() = tandem_core::config::TestStatus::Ok;
    let saved = t.app.save_sandbox_policy(&admin, candidate).await.unwrap();
    assert_eq!(saved.runner_tier, RunnerTier::Container);
    assert_eq!(
        t.probes.calls.lock().unwrap().runtime_runs,
        2,
        "probed on each save, never remembered"
    );
}
