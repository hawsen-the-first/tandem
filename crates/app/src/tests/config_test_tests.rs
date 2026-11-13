use super::*;
use crate::{JobOutcome, ProviderInput, SearchInput, SecretInput};
use tandem_core::config::{
    FailureReason, ProbeResult, ProbeSubject, RunStatus, SearchKind, TestStatus, TestTarget,
    ToolCalling,
};
use tandem_core::job::JobStatus;
use tandem_support::Secret;
use time::Duration;

const TTL: Duration = Duration::seconds(60);
const KEY: &str = "sk-test-opened-only-for-the-call";

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    (t, admin)
}

async fn provider(t: &TestApp, admin: &RequestContext) -> crate::ProviderView {
    t.app
        .create_provider(
            admin,
            ProviderInput {
                name: "Local".to_owned(),
                base_url: "http://localhost:11434/v1".to_owned(),
                key: SecretInput::Set(Secret::new(KEY)),
                models: vec![crate::ModelInput {
                    id: None,
                    name: "llama3".to_owned(),
                    label: String::new(),
                    declared_tool_calling: None,
                    declared_context_tokens: Some(1000),
                }],
            },
        )
        .await
        .unwrap()
}

async fn run_next(t: &TestApp) -> JobOutcome {
    t.app.process_next_job(TTL).await.unwrap()
}

fn job_count(t: &TestApp, status: JobStatus) -> usize {
    t.store
        .read(|s| s.jobs.iter().filter(|job| job.status == status).count())
}

#[tokio::test]
async fn rl_8_starting_a_test_queues_the_run_and_its_job_together() {
    let (t, admin) = setup().await;
    let view = provider(&t, &admin).await;
    let run = t
        .app
        .start_test(&admin, TestTarget::Provider(view.provider.id))
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Queued);
    assert_eq!(job_count(&t, JobStatus::Queued), 1);
    let fetched = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(fetched, run);
    let shown = t.app.get_provider(&admin, view.provider.id).await.unwrap();
    assert_eq!(
        shown.active_test,
        Some(run.id),
        "the screen can attach to it"
    );
}

#[tokio::test]
async fn rl_8_a_second_start_while_one_is_active_answers_already_running_with_its_id() {
    let (t, admin) = setup().await;
    let view = provider(&t, &admin).await;
    let target = TestTarget::Provider(view.provider.id);
    let first = t.app.start_test(&admin, target).await.unwrap();
    let second = t.app.start_test(&admin, target).await;
    assert!(matches!(second, Err(AppError::AlreadyRunning { run_id }) if run_id == first.id));
    assert_eq!(job_count(&t, JobStatus::Queued), 1, "no second job");
    // A different target is independent.
    t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
}

#[tokio::test]
async fn br_21_3_starting_and_reading_tests_is_for_admins_and_needs_a_real_provider() {
    let (t, admin) = setup().await;
    let (member_ctx, _) = member(&t, &admin, "mia@example.com").await;
    assert!(matches!(
        t.app.start_test(&member_ctx, TestTarget::Runtime).await,
        Err(AppError::NotFound)
    ));
    let unknown = TestTarget::Provider("00000000-0000-0000-0000-0000000000aa".parse().unwrap());
    assert!(matches!(
        t.app.start_test(&admin, unknown).await,
        Err(AppError::NotFound)
    ));
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    assert!(matches!(
        t.app.get_test_run(&member_ctx, run.id).await,
        Err(AppError::NotFound)
    ));
}

#[tokio::test]
async fn w17_a_provider_test_stores_results_and_detected_capabilities_and_audits_once() {
    let (t, admin) = setup().await;
    let view = provider(&t, &admin).await;
    let target = TestTarget::Provider(view.provider.id);
    let run = t.app.start_test(&admin, target).await.unwrap();
    assert!(matches!(
        run_next(&t).await,
        JobOutcome::Completed {
            kind: "config_test"
        }
    ));

    let finished = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(finished.status, RunStatus::Succeeded);
    assert!(finished.started_at.is_some() && finished.finished_at.is_some());
    assert_eq!(finished.results.len(), 2, "the provider and its model");
    assert!(
        finished
            .results
            .iter()
            .all(|result| result.status == TestStatus::Ok)
    );

    let shown = t.app.get_provider(&admin, view.provider.id).await.unwrap();
    assert_eq!(shown.last_test.as_ref().map(|run| run.id), Some(run.id));
    assert_eq!(shown.active_test, None);
    let capabilities = shown.models[0].model.capabilities;
    assert_eq!(capabilities.detected_tool_calling, ToolCalling::Supported);
    assert_eq!(capabilities.detected_context_tokens, Some(32_768));
    assert_eq!(
        capabilities.declared_context_tokens,
        Some(1000),
        "BR-14.5: declared is untouched"
    );
    assert_eq!(capabilities.context_tokens(), Some(1000));
    assert!(shown.models[0].disagrees_with_detection);

    assert_eq!(count_audit(&t, "config.test.run"), 1);
    let audit = t.store.read(|s| format!("{:?}", s.audit));
    assert!(!audit.contains(KEY));
}

#[tokio::test]
async fn br_20_1_the_runner_opens_the_key_for_the_probe_and_nothing_else_keeps_it() {
    let (t, admin) = setup().await;
    let view = provider(&t, &admin).await;
    let run = t
        .app
        .start_test(&admin, TestTarget::Provider(view.provider.id))
        .await
        .unwrap();
    run_next(&t).await;
    assert_eq!(
        t.probes.calls.lock().unwrap().provider_keys,
        vec![Some(KEY.to_owned())]
    );
    let stored = t
        .store
        .read(|s| format!("{:?}{:?}", s.config.test_runs, s.jobs));
    assert!(!stored.contains(KEY), "{run:?}");
}

#[tokio::test]
async fn w17_each_result_is_stored_and_the_failed_models_leave_capabilities_alone() {
    let (t, admin) = setup().await;
    let view = provider(&t, &admin).await;
    *t.probes.model_status.lock().unwrap() = TestStatus::UnknownModel;
    let run = t
        .app
        .start_test(&admin, TestTarget::Provider(view.provider.id))
        .await
        .unwrap();
    run_next(&t).await;
    let finished = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(
        finished.status,
        RunStatus::Succeeded,
        "problems are results, not failures"
    );
    let model_result = finished
        .results
        .iter()
        .find(|result| matches!(result.subject, ProbeSubject::Model(_)))
        .unwrap();
    assert_eq!(model_result.status, TestStatus::UnknownModel);
    let shown = t.app.get_provider(&admin, view.provider.id).await.unwrap();
    assert_eq!(
        shown.models[0].model.capabilities.detected_tool_calling,
        ToolCalling::Unknown
    );
    assert_eq!(count_audit(&t, "config.test.run"), 1);
    let outcome = t.store.read(|s| {
        s.audit
            .iter()
            .find(|entry| entry.event.event_type.as_str() == "config.test.run")
            .map(|entry| entry.event.outcome)
    });
    assert_eq!(outcome, Some(tandem_core::audit::AuditOutcome::Failure));
}

#[tokio::test]
async fn w20_a_search_test_without_settings_reports_not_configured() {
    let (t, admin) = setup().await;
    let run = t
        .app
        .start_test(&admin, TestTarget::Search(SearchKind::Brave))
        .await
        .unwrap();
    run_next(&t).await;
    let finished = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(finished.results[0].status, TestStatus::NotConfigured);
    assert_eq!(t.probes.calls.lock().unwrap().search_runs, 0);
}

#[tokio::test]
async fn w20_a_search_test_uses_the_saved_settings_and_is_the_last_result_for_activation() {
    let (t, admin) = setup().await;
    t.app
        .save_search(
            &admin,
            SearchInput {
                kind: SearchKind::Brave,
                base_url: None,
                key: SecretInput::Set(Secret::new("k")),
            },
        )
        .await
        .unwrap();
    t.app
        .start_test(&admin, TestTarget::Search(SearchKind::Brave))
        .await
        .unwrap();
    run_next(&t).await;
    assert_eq!(t.probes.calls.lock().unwrap().search_runs, 1);
    let activation = t
        .app
        .set_active_search(&admin, Some(SearchKind::Brave))
        .await
        .unwrap();
    assert!(!activation.untested, "a passing test removes the warning");
}

#[tokio::test]
async fn w21_a_connector_test_records_the_account_and_expiry_on_the_connector() {
    let (t, admin) = setup().await;
    t.app
        .save_connector_token(&admin, Secret::new("github_pat_x"))
        .await
        .unwrap();
    let mut result = ProbeResult::new(ProbeSubject::Connector, TestStatus::Ok);
    result.details.account_login = Some("octocat".to_owned());
    result.details.token_expires_at = Some(t.clock.now_for_assert() + Duration::days(30));
    *t.probes.connector_result.lock().unwrap() = result;
    t.app
        .start_test(
            &admin,
            TestTarget::Connector(tandem_core::config::ConnectorKind::Github),
        )
        .await
        .unwrap();
    run_next(&t).await;
    let view = t.app.get_connector(&admin).await.unwrap().unwrap();
    assert_eq!(view.connector.account_login.as_deref(), Some("octocat"));
    assert!(view.connector.token_expires_at.is_some());
    assert!(view.last_test.is_some());
}

#[tokio::test]
async fn w23_a_runtime_check_runs_as_a_test_run() {
    let (t, admin) = setup().await;
    *t.probes.runtime_status.lock().unwrap() = TestStatus::Unreachable;
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    run_next(&t).await;
    let finished = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(finished.results[0].status, TestStatus::Unreachable);
    assert_eq!(
        t.app
            .latest_test(&admin, TestTarget::Runtime)
            .await
            .unwrap()
            .unwrap()
            .id,
        run.id
    );
}

#[tokio::test]
async fn q1_a_target_whose_lock_is_held_is_retried_later_without_running_twice() {
    let (t, admin) = setup().await;
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    let held = t.locks.hold(&TestTarget::Runtime.key());
    assert!(matches!(run_next(&t).await, JobOutcome::Completed { .. }));
    assert_eq!(
        t.app.get_test_run(&admin, run.id).await.unwrap().status,
        RunStatus::Queued,
        "not started while another worker holds the target"
    );
    assert_eq!(t.probes.calls.lock().unwrap().runtime_runs, 0);
    assert_eq!(
        job_count(&t, JobStatus::Queued),
        1,
        "a delayed job was scheduled"
    );

    drop(held);
    t.clock.advance(Duration::seconds(6));
    run_next(&t).await;
    assert_eq!(
        t.app.get_test_run(&admin, run.id).await.unwrap().status,
        RunStatus::Succeeded
    );
    assert!(
        !t.locks.is_held(&TestTarget::Runtime.key()),
        "the lock is released after the run"
    );
}

#[tokio::test]
async fn d2_a_run_left_running_by_a_dead_worker_starts_again_from_nothing() {
    let (t, admin) = setup().await;
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    t.store.write(|s| {
        let stored = &mut s.config.test_runs[0];
        stored.start(stored.requested_at).unwrap();
        stored
            .record(ProbeResult::new(ProbeSubject::Runtime, TestStatus::Timeout))
            .unwrap();
    });
    run_next(&t).await;
    let finished = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(finished.status, RunStatus::Succeeded);
    assert_eq!(finished.results.len(), 1);
    assert_eq!(
        finished.results[0].status,
        TestStatus::Ok,
        "the stale partial result is gone"
    );
}

#[tokio::test]
async fn d2_a_crash_retries_the_run_and_the_last_failed_attempt_fails_it() {
    let (t, admin) = setup().await;
    t.app
        .save_connector_token(&admin, Secret::new("github_pat_x"))
        .await
        .unwrap();
    // A token the vault cannot open makes every attempt fail.
    t.store
        .write(|s| s.config.connectors[0].1 = b"corrupt".to_vec());
    let run = t
        .app
        .start_test(
            &admin,
            TestTarget::Connector(tandem_core::config::ConnectorKind::Github),
        )
        .await
        .unwrap();

    assert!(matches!(run_next(&t).await, JobOutcome::Retrying { .. }));
    let after_first = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(
        after_first.status,
        RunStatus::Queued,
        "back to queued for the retry"
    );
    assert!(after_first.results.is_empty());

    loop {
        t.clock.advance(Duration::minutes(10));
        match run_next(&t).await {
            JobOutcome::Retrying { .. } => {}
            JobOutcome::Failed { .. } => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    let failed = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(failed.status, RunStatus::Failed);
    assert_eq!(failed.failure, Some(FailureReason::CouldNotComplete));
    assert_eq!(count_audit(&t, "config.test.run"), 1);
    // The target is free again.
    t.app
        .start_test(
            &admin,
            TestTarget::Connector(tandem_core::config::ConnectorKind::Github),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn rl_9_a_failed_audit_write_after_a_run_does_not_undo_the_result() {
    let (t, admin) = setup().await;
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    t.store.write(|s| s.fail_audit = true);
    assert!(matches!(run_next(&t).await, JobOutcome::Completed { .. }));
    assert_eq!(
        t.app.get_test_run(&admin, run.id).await.unwrap().status,
        RunStatus::Succeeded
    );
    assert_eq!(count_audit(&t, "config.test.run"), 0);
}

#[tokio::test]
async fn d2_a_run_nobody_picked_up_in_two_minutes_is_failed_and_the_target_freed() {
    let (t, admin) = setup().await;
    let run = t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
    t.clock.advance(Duration::seconds(100));
    assert_eq!(t.app.fail_stuck_test_runs().await.unwrap(), 0, "not yet");
    t.clock.advance(Duration::seconds(30));
    assert_eq!(t.app.fail_stuck_test_runs().await.unwrap(), 1);
    let failed = t.app.get_test_run(&admin, run.id).await.unwrap();
    assert_eq!(failed.status, RunStatus::Failed);
    assert_eq!(failed.failure, Some(FailureReason::NoWorkerAvailable));
    assert_eq!(count_audit(&t, "config.test.run"), 1);
    t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
}

#[tokio::test]
async fn d3_pruning_keeps_the_latest_finished_run_per_target_and_the_recent_ten() {
    let (t, admin) = setup().await;
    for _ in 0..13 {
        t.app.start_test(&admin, TestTarget::Runtime).await.unwrap();
        run_next(&t).await;
        t.clock.advance(Duration::minutes(1));
    }
    let mut tx = tandem_ports::Store::begin(&t.store).await.unwrap();
    let deleted = tx
        .test_runs()
        .prune(10, t.clock.now_for_assert() - Duration::days(30))
        .await
        .unwrap();
    assert_eq!(deleted, 3);
    tx.commit().await.unwrap();
    assert_eq!(t.store.read(|s| s.config.test_runs.len()), 10);
}
