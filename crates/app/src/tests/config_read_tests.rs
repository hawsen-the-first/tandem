use super::*;
use crate::{DefaultsInput, ModelInput, ProviderInput, SearchInput, SecretInput};
use tandem_core::config::SearchKind;
use tandem_core::sandbox_policy::{NetworkRule, SandboxPolicy, ToolId};
use tandem_support::Secret;
use time::Duration;

const KEY: &str = "sk-test-only-in-memory";

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    (t, admin)
}

fn input(name: &str) -> ProviderInput {
    ProviderInput {
        name: name.to_owned(),
        base_url: "http://localhost:11434/v1".to_owned(),
        key: SecretInput::Set(Secret::new(KEY)),
        models: vec![ModelInput {
            id: None,
            name: "llama3".to_owned(),
            label: String::new(),
            declared_tool_calling: None,
            declared_context_tokens: None,
        }],
    }
}

#[tokio::test]
async fn w24_list_models_and_resolve_a_model_with_the_key_opened_in_memory() {
    let (t, admin) = setup().await;
    let view = t.app.create_provider(&admin, input("Local")).await.unwrap();
    let listed = t.app.config_list_models().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].provider_name, "Local");
    assert!(!listed[0].warnings.is_empty());

    let resolved = t
        .app
        .config_resolve_model(view.models[0].model.id)
        .await
        .unwrap();
    assert_eq!(resolved.provider.id, view.provider.id);
    assert_eq!(resolved.connection.api_key.as_ref().unwrap().expose(), KEY);
    assert!(
        !format!("{resolved:?}").contains(KEY),
        "D6: the connection cannot print its key"
    );

    let unknown = "00000000-0000-0000-0000-0000000000ee".parse().unwrap();
    assert!(matches!(
        t.app.config_resolve_model(unknown).await,
        Err(AppError::NotFound)
    ));
}

#[tokio::test]
async fn w24_defaults_search_and_github_credentials_reach_the_runtime() {
    let (t, admin) = setup().await;
    assert!(t.app.config_active_search().await.unwrap().is_none());
    assert!(t.app.config_github_credentials().await.unwrap().is_none());

    let view = t.app.create_provider(&admin, input("Local")).await.unwrap();
    let model = view.models[0].model.id;
    t.app
        .set_defaults(
            &admin,
            DefaultsInput {
                orchestrator: Some(model),
                agents: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        t.app.config_defaults().await.unwrap().orchestrator,
        Some(model)
    );

    t.app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Tavily,
                base_url: None,
                key: SecretInput::Set(Secret::new("tvly")),
            },
        )
        .await
        .unwrap();
    t.app
        .set_active_search(&admin, Some(SearchKind::Tavily))
        .await
        .unwrap();
    let search = t.app.config_active_search().await.unwrap().unwrap();
    assert_eq!(search.kind, SearchKind::Tavily);
    assert_eq!(search.api_key.unwrap().expose(), "tvly");

    t.app
        .save_connector_token(&admin, Secret::new("github_pat_q"))
        .await
        .unwrap();
    let github = t.app.config_github_credentials().await.unwrap().unwrap();
    assert_eq!(github.token.expose(), "github_pat_q");
}

#[tokio::test]
async fn a12_a_snapshot_is_reused_for_under_five_seconds_then_read_again() {
    let (t, admin) = setup().await;
    t.app.create_provider(&admin, input("Local")).await.unwrap();
    assert_eq!(t.app.config_list_models().await.unwrap().len(), 1);

    // Change the store behind the service's back, as another process would.
    t.store.write(|s| s.config.models.clear());
    t.clock.advance(Duration::seconds(4));
    assert_eq!(
        t.app.config_list_models().await.unwrap().len(),
        1,
        "still within five seconds"
    );
    t.clock.advance(Duration::seconds(1));
    assert!(
        t.app.config_list_models().await.unwrap().is_empty(),
        "five seconds is the limit"
    );
}

#[tokio::test]
async fn a12_a_local_write_is_visible_to_local_reads_at_once() {
    let (t, admin) = setup().await;
    let view = t.app.create_provider(&admin, input("Local")).await.unwrap();
    assert_eq!(t.app.config_list_models().await.unwrap().len(), 1);
    t.app
        .delete_provider(&admin, view.provider.id)
        .await
        .unwrap();
    assert!(
        t.app.config_list_models().await.unwrap().is_empty(),
        "the cache was cleared"
    );
}

#[tokio::test]
async fn br_18_8_the_effective_policy_is_the_instance_policy_or_a_session_policy_tightened_to_it() {
    let (t, admin) = setup().await;
    let mut instance = SandboxPolicy::factory_default();
    instance.limits.wall_clock_seconds = 300;
    let saved = t.app.save_sandbox_policy(&admin, instance).await.unwrap();

    let effective = t.app.config_effective_policy(None).await.unwrap();
    assert_eq!(effective, saved);

    let mut session = SandboxPolicy::factory_default();
    session.scope = tandem_core::sandbox_policy::PolicyScope::Session;
    session.limits.wall_clock_seconds = 3000;
    session.limits.max_files_written = 10;
    session.tool_allow_list.remove(&ToolId::WebSearch);
    session.network = NetworkRule::None;
    let clamped = t.app.config_effective_policy(Some(&session)).await.unwrap();
    assert_eq!(
        clamped.limits.wall_clock_seconds, 300,
        "cannot exceed the ceiling"
    );
    assert_eq!(clamped.limits.max_files_written, 10, "may tighten");
    assert!(
        !clamped.tool_allow_list.contains(&ToolId::WebSearch),
        "a narrower tool list is kept"
    );
    assert_eq!(
        clamped.version, saved.version,
        "D5: records the instance version it captured"
    );
}
