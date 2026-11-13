//! Instance administration over HTTP, through the real router and pipeline over the in-memory
//! application: access, CSRF, secrets never returned, validation, defaults, search, the connector,
//! the sandbox policy and the test-run lifecycle.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Harness, User, delete, get, post, put};
use serde_json::{Value, json};
use tandem_app::JobOutcome;
use tandem_core::config::TestStatus;
use time::Duration;

const KEY: &str = "sk-test-never-appears-in-any-response";
const TOKEN: &str = "github_pat_never_appears_in_any_response";

async fn admin(h: &Harness) -> User {
    h.setup_admin("ada@example.test").await
}

fn provider_body(name: &str) -> Value {
    json!({
        "name": name,
        "base_url": "http://localhost:11434/v1",
        "api_key": KEY,
        "models": [{ "name": "llama3" }, { "name": "qwen", "declared_context_tokens": 16000 }],
    })
}

async fn create_provider(h: &Harness, admin: &User, name: &str) -> Value {
    let reply = h
        .send(
            post("/api/admin/providers")
                .signed_in(admin)
                .json(&provider_body(name)),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    reply.json()
}

async fn run_job(h: &Harness) {
    let outcome =
        h.t.app
            .process_next_job(Duration::seconds(60))
            .await
            .unwrap();
    assert!(
        matches!(
            outcome,
            JobOutcome::Completed {
                kind: "config_test"
            }
        ),
        "{outcome:?}"
    );
}

const ADMIN_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/admin/providers"),
    ("POST", "/api/admin/providers"),
    (
        "GET",
        "/api/admin/providers/00000000-0000-0000-0000-000000000001",
    ),
    (
        "PUT",
        "/api/admin/providers/00000000-0000-0000-0000-000000000001",
    ),
    (
        "DELETE",
        "/api/admin/providers/00000000-0000-0000-0000-000000000001",
    ),
    (
        "GET",
        "/api/admin/providers/00000000-0000-0000-0000-000000000001/removal-impact",
    ),
    (
        "POST",
        "/api/admin/providers/00000000-0000-0000-0000-000000000001/test",
    ),
    ("GET", "/api/admin/model-defaults"),
    ("PUT", "/api/admin/model-defaults"),
    ("GET", "/api/admin/search"),
    ("PUT", "/api/admin/search/kinds/brave"),
    ("POST", "/api/admin/search/kinds/brave/test"),
    ("PUT", "/api/admin/search/active"),
    ("GET", "/api/admin/connectors/github"),
    ("PUT", "/api/admin/connectors/github"),
    ("DELETE", "/api/admin/connectors/github"),
    ("POST", "/api/admin/connectors/github/test"),
    ("GET", "/api/admin/sandbox-policy"),
    ("PUT", "/api/admin/sandbox-policy"),
    ("POST", "/api/admin/runtime/check"),
    (
        "GET",
        "/api/admin/test-runs/00000000-0000-0000-0000-000000000001",
    ),
];

fn call(method: &str, path: &str) -> common::Call {
    match method {
        "GET" => get(path),
        "POST" => post(path),
        "PUT" => put(path),
        _ => delete(path),
    }
}

#[tokio::test]
async fn br_21_3_anonymous_callers_are_unauthenticated_and_members_are_told_the_route_is_absent() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let member = h.add_member(&admin, "mia@example.test").await;
    for (method, path) in ADMIN_ROUTES {
        let anonymous = h.send(call(method, path).origin().json(&json!({}))).await;
        assert_eq!(
            anonymous.status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
        let as_member = h
            .send(call(method, path).signed_in(&member).json(&json!({})))
            .await;
        assert_eq!(
            as_member.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            as_member.text
        );
        assert_eq!(as_member.error_code(), "not_found");
    }
}

#[tokio::test]
async fn an_admin_who_has_not_finished_mfa_enrolment_cannot_reach_them() {
    let h = Harness::new();
    let setup = h
        .send(post("/api/setup").origin().json(&json!({
            "display_name": "Ada Admin", "email": "ada@example.test", "password": common::PASSWORD
        })))
        .await;
    let enrolling = h.identify(&setup.session_cookie().unwrap()).await;
    let reply = h
        .send(get("/api/admin/providers").signed_in(&enrolling))
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.error_code(), "mfa_enrolment_required");
}

#[tokio::test]
async fn se_9_changes_need_the_csrf_token() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let reply = h
        .send(
            post("/api/admin/providers")
                .cookie(&admin.cookie)
                .origin()
                .json(&provider_body("Local")),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.error_code(), "csrf");
    let listed = h
        .send(get("/api/admin/providers").cookie(&admin.cookie))
        .await;
    assert_eq!(
        listed.json().as_array().unwrap().len(),
        0,
        "nothing was created"
    );
}

#[tokio::test]
async fn br_20_2_no_response_ever_carries_a_secret() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let mut transcript = String::new();
    let mut record = |reply: &common::Reply| transcript.push_str(&reply.text);

    let created = h
        .send(
            post("/api/admin/providers")
                .signed_in(&admin)
                .json(&provider_body("Local")),
        )
        .await;
    record(&created);
    let id = created.json()["id"].as_str().unwrap().to_owned();
    assert_eq!(created.json()["api_key_saved"], true);
    assert!(created.json()["api_key_saved_at"].is_string());
    assert!(created.json().get("api_key").is_none());

    for path in [
        "/api/admin/providers".to_owned(),
        format!("/api/admin/providers/{id}"),
        format!("/api/admin/providers/{id}/removal-impact"),
        "/api/admin/model-defaults".to_owned(),
    ] {
        record(&h.send(get(&path).signed_in(&admin)).await);
    }
    let updated = h
        .send(
            put(&format!("/api/admin/providers/{id}"))
                .signed_in(&admin)
                .json(&json!({"name": "Local", "base_url": "http://localhost:11434/v1", "models": []})),
        )
        .await;
    record(&updated);
    assert_eq!(
        updated.json()["api_key_saved"],
        true,
        "an absent key means unchanged"
    );

    record(
        &h.send(
            put("/api/admin/search/kinds/brave")
                .signed_in(&admin)
                .json(&json!({"api_key": KEY})),
        )
        .await,
    );
    record(&h.send(get("/api/admin/search").signed_in(&admin)).await);
    record(
        &h.send(
            put("/api/admin/connectors/github")
                .signed_in(&admin)
                .json(&json!({"token": TOKEN})),
        )
        .await,
    );
    record(
        &h.send(get("/api/admin/connectors/github").signed_in(&admin))
            .await,
    );
    record(
        &h.send(get("/api/admin/audit?limit=200").signed_in(&admin))
            .await,
    );
    record(
        &h.send(delete(&format!("/api/admin/providers/{id}")).signed_in(&admin))
            .await,
    );

    assert!(!transcript.contains(KEY), "a key leaked into a response");
    assert!(
        !transcript.contains(TOKEN),
        "a token leaked into a response"
    );
}

#[tokio::test]
async fn br_20_3_a_key_can_be_kept_replaced_or_removed_explicitly() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let created = create_provider(&h, &admin, "Local").await;
    let id = created["id"].as_str().unwrap();
    let edit = |body: Value| {
        let h = &h;
        let admin = &admin;
        async move {
            h.send(
                put(&format!("/api/admin/providers/{id}"))
                    .signed_in(admin)
                    .json(&body),
            )
            .await
        }
    };
    let base = json!({"name": "Local", "base_url": "http://localhost:11434/v1", "models": []});

    let kept = edit(json!({"name": "Local", "base_url": "http://localhost:11434/v1", "api_key": "  ", "models": []})).await;
    assert_eq!(
        kept.json()["api_key_saved"],
        true,
        "whitespace is an empty field"
    );
    let mut remove = base.clone();
    remove["remove_api_key"] = json!(true);
    let removed = edit(remove).await;
    assert_eq!(removed.json()["api_key_saved"], false);
    assert!(removed.json()["api_key_saved_at"].is_null());
    let mut set = base;
    set["api_key"] = json!("another-key");
    assert_eq!(edit(set).await.json()["api_key_saved"], true);
}

#[tokio::test]
async fn br_19_7_url_refusals_come_back_with_the_reason_and_the_field() {
    let h = Harness::new();
    let admin = admin(&h).await;
    for (url, expect) in [
        ("http://169.254.169.254/latest", "not allowed"),
        ("http://[fd00:ec2::254]/", "not allowed"),
        ("http://example.com/v1", "https://"),
        ("ftp://example.com", "http:// or https://"),
        ("https://user:pw@example.com", "user name"),
        ("https://example.com/v1?x=1", "after ?"),
    ] {
        let reply = h
            .send(post("/api/admin/providers").signed_in(&admin).json(&json!({
                "name": "Bad", "base_url": url, "models": []
            })))
            .await;
        assert_eq!(
            reply.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{url}: {}",
            reply.text
        );
        let field = &reply.json()["error"]["fields"][0];
        assert_eq!(field["field"], "base_url", "{url}");
        assert!(
            field["message"].as_str().unwrap().contains(expect),
            "{url}: {field}"
        );
    }
    let listed = h.send(get("/api/admin/providers").signed_in(&admin)).await;
    assert!(listed.json().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn br_14_3_every_field_problem_is_named_and_malformed_ids_are_not_found() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let reply = h
        .send(post("/api/admin/providers").signed_in(&admin).json(&json!({
            "name": " ", "base_url": "", "models": [{"name": "a"}, {"name": "a"}]
        })))
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: Vec<String> = reply.json()["error"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["field"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        ["name", "base_url", "models"]
            .iter()
            .all(|expected| fields.iter().any(|field| field == expected)),
        "{fields:?}"
    );

    let not_an_id = h
        .send(get("/api/admin/providers/nope").signed_in(&admin))
        .await;
    assert_eq!(not_an_id.status, StatusCode::NOT_FOUND);
    let wrong_tool_calling = h
        .send(post("/api/admin/providers").signed_in(&admin).json(&json!({
            "name": "X", "base_url": "http://localhost:1", "models": [{"name": "a", "declared_tool_calling": "maybe"}]
        })))
        .await;
    assert_eq!(wrong_tool_calling.status, StatusCode::UNPROCESSABLE_ENTITY);
    let duplicate = h
        .send(
            post("/api/admin/providers")
                .signed_in(&admin)
                .json(&provider_body("Dup")),
        )
        .await;
    assert_eq!(duplicate.status, StatusCode::CREATED);
    let again = h
        .send(
            post("/api/admin/providers")
                .signed_in(&admin)
                .json(&provider_body("dup")),
        )
        .await;
    assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(again.json()["error"]["fields"][0]["field"], "name");
}

#[tokio::test]
async fn w18_removing_a_provider_reports_what_it_clears_and_clears_it() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let created = create_provider(&h, &admin, "Local").await;
    let id = created["id"].as_str().unwrap();
    let model = created["models"][0]["id"].as_str().unwrap();
    let defaults = h
        .send(
            put("/api/admin/model-defaults")
                .signed_in(&admin)
                .json(&json!({
                    "orchestrator_model_id": model, "agents_model_id": model
                })),
        )
        .await;
    assert_eq!(defaults.status, StatusCode::OK, "{}", defaults.text);
    let shown = defaults.json();
    assert_eq!(shown["orchestrator"]["provider_name"], "Local", "BR-15.5");
    let warning_codes: Vec<&str> = shown["orchestrator"]["model"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warning| warning["code"].as_str().unwrap())
        .collect();
    assert!(
        warning_codes.contains(&"tool_calling_unknown"),
        "{warning_codes:?}"
    );

    let impact = h
        .send(get(&format!("/api/admin/providers/{id}/removal-impact")).signed_in(&admin))
        .await;
    assert_eq!(
        impact.json()["cleared_defaults"],
        json!(["orchestrator", "agents"])
    );
    assert_eq!(impact.json()["model_count"], 2);

    let removed = h
        .send(delete(&format!("/api/admin/providers/{id}")).signed_in(&admin))
        .await;
    assert_eq!(removed.status, StatusCode::OK);
    let after = h
        .send(get("/api/admin/model-defaults").signed_in(&admin))
        .await
        .json();
    assert!(after["orchestrator"].is_null() && after["agents"].is_null());
    let gone = h
        .send(get(&format!("/api/admin/providers/{id}")).signed_in(&admin))
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn br_15_1_a_default_must_be_a_listed_model() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let unknown = h
        .send(
            put("/api/admin/model-defaults")
                .signed_in(&admin)
                .json(&json!({
                    "orchestrator_model_id": "00000000-0000-0000-0000-0000000000ff"
                })),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        unknown.json()["error"]["fields"][0]["field"],
        "orchestrator"
    );
    let malformed = h
        .send(
            put("/api/admin/model-defaults")
                .signed_in(&admin)
                .json(&json!({"agents_model_id": "x"})),
        )
        .await;
    assert_eq!(
        malformed.json()["error"]["fields"][0]["field"],
        "agents_model_id"
    );
}

#[tokio::test]
async fn rl_8_a_test_is_accepted_runs_in_the_background_and_a_second_start_answers_409_with_its_id()
{
    let h = Harness::new();
    let admin = admin(&h).await;
    let created = create_provider(&h, &admin, "Local").await;
    let id = created["id"].as_str().unwrap();
    let start = h
        .send(post(&format!("/api/admin/providers/{id}/test")).signed_in(&admin))
        .await;
    assert_eq!(start.status, StatusCode::ACCEPTED, "{}", start.text);
    let run = start.json();
    assert_eq!(run["status"], "queued");
    let run_id = run["id"].as_str().unwrap().to_owned();

    let second = h
        .send(post(&format!("/api/admin/providers/{id}/test")).signed_in(&admin))
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    assert_eq!(second.error_code(), "already_running");
    assert_eq!(
        second.json()["error"]["run_id"],
        run_id,
        "the screen attaches to it"
    );

    let queued = h
        .send(get(&format!("/api/admin/test-runs/{run_id}")).signed_in(&admin))
        .await;
    assert_eq!(queued.json()["status"], "queued");
    let shown = h
        .send(get(&format!("/api/admin/providers/{id}")).signed_in(&admin))
        .await;
    assert_eq!(shown.json()["active_test_id"], run_id);

    run_job(&h).await;
    let finished = h
        .send(get(&format!("/api/admin/test-runs/{run_id}")).signed_in(&admin))
        .await
        .json();
    assert_eq!(finished["status"], "succeeded");
    assert_eq!(
        finished["results"].as_array().unwrap().len(),
        3,
        "the provider and two models"
    );
    for result in finished["results"].as_array().unwrap() {
        assert_eq!(result["status"], "ok");
        assert_eq!(result["message"], TestStatus::Ok.message());
    }
    let provider = h
        .send(get(&format!("/api/admin/providers/{id}")).signed_in(&admin))
        .await
        .json();
    assert_eq!(provider["last_test"]["id"], run_id);
    assert!(provider["active_test_id"].is_null());
    assert_eq!(provider["models"][0]["detected_tool_calling"], "supported");
    assert_eq!(provider["models"][0]["detected_context_tokens"], 32768);
    assert_eq!(provider["models"][1]["declared_context_tokens"], 16000);
    assert_eq!(
        provider["models"][1]["context_tokens"], 16000,
        "declared wins"
    );
    assert_eq!(provider["models"][1]["disagrees_with_detection"], true);

    let again = h
        .send(post(&format!("/api/admin/providers/{id}/test")).signed_in(&admin))
        .await;
    assert_eq!(
        again.status,
        StatusCode::ACCEPTED,
        "free again once finished"
    );
    let unknown = h
        .send(
            post("/api/admin/providers/00000000-0000-0000-0000-0000000000aa/test")
                .signed_in(&admin),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn se_22_test_results_never_contain_remote_text() {
    let h = Harness::new();
    let admin = admin(&h).await;
    *h.t.probes.model_status.lock().unwrap() = TestStatus::Unauthorised;
    let created = create_provider(&h, &admin, "Local").await;
    let id = created["id"].as_str().unwrap();
    h.send(post(&format!("/api/admin/providers/{id}/test")).signed_in(&admin))
        .await;
    run_job(&h).await;
    let provider = h
        .send(get(&format!("/api/admin/providers/{id}")).signed_in(&admin))
        .await
        .json();
    let model_results: Vec<&Value> = provider["last_test"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|result| result["subject"] == "model")
        .collect();
    assert_eq!(model_results.len(), 2);
    for result in model_results {
        assert_eq!(result["status"], "unauthorised");
        assert_eq!(result["message"], TestStatus::Unauthorised.message());
    }
}

#[tokio::test]
async fn se_24_test_starts_have_their_own_rate_limit_and_other_routes_are_unaffected() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let mut statuses = Vec::new();
    for _ in 0..12 {
        let reply = h
            .send(post("/api/admin/runtime/check").signed_in(&admin))
            .await;
        statuses.push(reply.status);
    }
    assert_eq!(statuses[0], StatusCode::ACCEPTED);
    assert!(
        statuses[1..10]
            .iter()
            .all(|status| *status == StatusCode::CONFLICT),
        "{statuses:?}"
    );
    assert_eq!(statuses[10], StatusCode::TOO_MANY_REQUESTS, "{statuses:?}");
    assert_eq!(statuses[11], StatusCode::TOO_MANY_REQUESTS);
    let other = h.send(get("/api/admin/providers").signed_in(&admin)).await;
    assert_eq!(
        other.status,
        StatusCode::OK,
        "the general bucket is separate"
    );
    let limited = h
        .send(post("/api/admin/runtime/check").signed_in(&admin))
        .await;
    assert!(limited.header("retry-after").parse::<u64>().unwrap() >= 1);
}

#[tokio::test]
async fn w20_search_providers_are_saved_tested_and_one_is_activated() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let overview = h
        .send(get("/api/admin/search").signed_in(&admin))
        .await
        .json();
    assert_eq!(overview["kinds"].as_array().unwrap().len(), 3);
    assert!(overview["active"].is_null());

    let needs_key = h
        .send(
            put("/api/admin/search/active")
                .signed_in(&admin)
                .json(&json!({"kind": "brave"})),
        )
        .await;
    assert_eq!(needs_key.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(needs_key.json()["error"]["fields"][0]["field"], "kind");

    let saved = h
        .send(
            put("/api/admin/search/kinds/brave")
                .signed_in(&admin)
                .json(&json!({"api_key": KEY})),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.text);
    let brave = saved.json()["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kind| kind["kind"] == "brave")
        .cloned()
        .unwrap();
    assert_eq!(
        (brave["configured"].clone(), brave["api_key_saved"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(brave["needs_key"], true);

    let activated = h
        .send(
            put("/api/admin/search/active")
                .signed_in(&admin)
                .json(&json!({"kind": "brave"})),
        )
        .await;
    assert_eq!(
        activated.json(),
        json!({"active": "brave", "untested": true})
    );

    let test = h
        .send(post("/api/admin/search/kinds/brave/test").signed_in(&admin))
        .await;
    assert_eq!(test.status, StatusCode::ACCEPTED);
    run_job(&h).await;
    let again = h
        .send(
            put("/api/admin/search/active")
                .signed_in(&admin)
                .json(&json!({"kind": "brave"})),
        )
        .await;
    assert_eq!(again.json()["untested"], false);

    let off = h
        .send(
            put("/api/admin/search/active")
                .signed_in(&admin)
                .json(&json!({})),
        )
        .await;
    assert!(off.json()["active"].is_null());
    let bad_kind = h
        .send(
            put("/api/admin/search/kinds/google")
                .signed_in(&admin)
                .json(&json!({})),
        )
        .await;
    assert_eq!(bad_kind.status, StatusCode::NOT_FOUND);
    let bad_active = h
        .send(
            put("/api/admin/search/active")
                .signed_in(&admin)
                .json(&json!({"kind": "google"})),
        )
        .await;
    assert_eq!(bad_active.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn w21_the_github_token_is_saved_tested_and_removed() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let before = h
        .send(get("/api/admin/connectors/github").signed_in(&admin))
        .await;
    assert_eq!(before.status, StatusCode::OK);
    assert_eq!(before.json()["configured"], false);

    let saved = h
        .send(
            put("/api/admin/connectors/github")
                .signed_in(&admin)
                .json(&json!({"token": TOKEN})),
        )
        .await;
    assert_eq!(saved.json()["configured"], true);
    assert!(saved.json()["saved_at"].is_string());
    let blank = h
        .send(
            put("/api/admin/connectors/github")
                .signed_in(&admin)
                .json(&json!({"token": "  "})),
        )
        .await;
    assert_eq!(blank.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(blank.json()["error"]["fields"][0]["field"], "token");

    let test = h
        .send(post("/api/admin/connectors/github/test").signed_in(&admin))
        .await;
    assert_eq!(test.status, StatusCode::ACCEPTED);
    run_job(&h).await;
    let status = h
        .send(get("/api/admin/connectors/github").signed_in(&admin))
        .await
        .json();
    assert_eq!(status["last_test"]["status"], "succeeded");

    let removed = h
        .send(delete("/api/admin/connectors/github").signed_in(&admin))
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let again = h
        .send(delete("/api/admin/connectors/github").signed_in(&admin))
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn w22_the_sandbox_policy_is_read_with_its_ranges_validated_and_saved() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let view = h
        .send(get("/api/admin/sandbox-policy").signed_in(&admin))
        .await
        .json();
    assert_eq!(view["policy"]["runner_tier"], "in_process");
    assert_eq!(view["policy"]["version"], 1);
    assert_eq!(view["policy"]["network"]["mode"], "none");
    assert_eq!(view["factory"], view["policy"]);
    assert_eq!(view["ranges"].as_array().unwrap().len(), 6);
    let command_run = view["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["id"] == "command_run")
        .unwrap();
    assert_eq!(command_run["can_be_enabled"], false);

    let mut body = json!({
        "runner_tier": "in_process",
        "tool_allow_list": view["policy"]["tool_allow_list"],
        "network": {"mode": "allow_hosts", "hosts": ["*.example.com", "api.test"]},
        "limits": view["policy"]["limits"],
    });
    body["limits"]["wall_clock_seconds"] = json!(900);
    let saved = h
        .send(
            put("/api/admin/sandbox-policy")
                .signed_in(&admin)
                .json(&body),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.text);
    assert_eq!(saved.json()["policy"]["version"], 2);
    assert_eq!(saved.json()["policy"]["limits"]["wall_clock_seconds"], 900);
    assert_eq!(
        saved.json()["policy"]["network"]["hosts"],
        json!(["*.example.com", "api.test"])
    );

    let mut bad = body.clone();
    bad["tool_allow_list"] = json!(["command_run", "teleport"]);
    bad["limits"]["wall_clock_seconds"] = json!(1);
    bad["network"] = json!({"mode": "allow_hosts", "hosts": ["https://x.example/path"]});
    let refused = h
        .send(
            put("/api/admin/sandbox-policy")
                .signed_in(&admin)
                .json(&bad),
        )
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: Vec<String> = refused.json()["error"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["field"].as_str().unwrap().to_owned())
        .collect();
    for expected in ["tool_allow_list", "network"] {
        assert!(fields.iter().any(|field| field == expected), "{fields:?}");
    }
    let unchanged = h
        .send(get("/api/admin/sandbox-policy").signed_in(&admin))
        .await
        .json();
    assert_eq!(
        unchanged["policy"]["version"], 2,
        "a refused save changes nothing"
    );
}

#[tokio::test]
async fn br_18_2_the_container_tier_is_refused_until_the_runtime_answers() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let view = h
        .send(get("/api/admin/sandbox-policy").signed_in(&admin))
        .await
        .json();
    let mut body = json!({
        "runner_tier": "container",
        "tool_allow_list": view["policy"]["tool_allow_list"],
        "network": {"mode": "none"},
        "limits": view["policy"]["limits"],
    });
    *h.t.probes.runtime_status.lock().unwrap() = TestStatus::Unreachable;
    let refused = h
        .send(
            put("/api/admin/sandbox-policy")
                .signed_in(&admin)
                .json(&body),
        )
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.json()["error"]["fields"][0]["field"], "runner_tier");

    *h.t.probes.runtime_status.lock().unwrap() = TestStatus::Ok;
    let saved = h
        .send(
            put("/api/admin/sandbox-policy")
                .signed_in(&admin)
                .json(&body),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.text);
    assert_eq!(saved.json()["policy"]["runner_tier"], "container");
    body["runner_tier"] = json!("hypervisor");
    let unknown = h
        .send(
            put("/api/admin/sandbox-policy")
                .signed_in(&admin)
                .json(&body),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn w23_a_runtime_check_is_a_test_run() {
    let h = Harness::new();
    let admin = admin(&h).await;
    *h.t.probes.runtime_status.lock().unwrap() = TestStatus::Unreachable;
    let started = h
        .send(post("/api/admin/runtime/check").signed_in(&admin))
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED);
    let run_id = started.json()["id"].as_str().unwrap().to_owned();
    run_job(&h).await;
    let run = h
        .send(get(&format!("/api/admin/test-runs/{run_id}")).signed_in(&admin))
        .await
        .json();
    assert_eq!(run["target"], "runtime");
    assert_eq!(run["results"][0]["subject"], "runtime");
    assert_eq!(run["results"][0]["status"], "unreachable");
}

#[tokio::test]
async fn br_21_2_every_change_and_test_is_audited_with_before_and_after_and_no_secret() {
    let h = Harness::new();
    let admin = admin(&h).await;
    let created = create_provider(&h, &admin, "Local").await;
    let id = created["id"].as_str().unwrap();
    h.send(
        put(&format!("/api/admin/providers/{id}"))
            .signed_in(&admin)
            .json(&json!({"name": "Renamed", "base_url": "http://localhost:11434/v1", "remove_api_key": true, "models": []})),
    )
    .await;
    h.send(post(&format!("/api/admin/providers/{id}/test")).signed_in(&admin))
        .await;
    run_job(&h).await;

    let log = h
        .send(get("/api/admin/audit?limit=200").signed_in(&admin))
        .await
        .json();
    let entries = log["entries"].as_array().unwrap();
    let find = |event_type: &str| {
        entries
            .iter()
            .find(|entry| entry["event_type"] == event_type)
            .unwrap_or_else(|| panic!("no {event_type} entry"))
            .clone()
    };
    assert_eq!(find("config.provider.created")["details"]["api_key"], "set");
    let updated = find("config.provider.updated");
    assert_eq!(updated["details"]["name_before"], "Local");
    assert_eq!(updated["details"]["name_after"], "Renamed");
    assert_eq!(updated["details"]["api_key_before"], "set");
    assert_eq!(updated["details"]["api_key_after"], "not set");
    let test = find("config.test.run");
    assert_eq!(test["outcome"], "success");
    assert_eq!(test["actor_label"], "Ada Admin");
    assert!(!log.to_string().contains(KEY));
}
