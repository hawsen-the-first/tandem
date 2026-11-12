//! The request pipeline: headers, request IDs, rate limiting, body limit, panics, CSRF, Origin,
//! sessions, authorisation, health and metrics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get as axum_get;
use common::{Harness, Options, PASSWORD, delete, get, post, put};
use serde_json::json;
use std::sync::atomic::Ordering;
use tandem_api::pipeline::catch_panic_response;
use tower::ServiceExt;
use tower_http::catch_panic::CatchPanicLayer;

async fn boom() -> &'static str {
    panic!("secret internal detail")
}

// ---- headers and request identity ---------------------------------------------------------

#[tokio::test]
async fn every_response_carries_the_security_headers() {
    let harness = Harness::new();
    for (path, expected_status) in [
        ("/healthz", StatusCode::OK),
        ("/api/auth/me", StatusCode::UNAUTHORIZED),
        ("/api/nothing-here", StatusCode::NOT_FOUND),
        ("/not-an-api-path", StatusCode::NOT_FOUND),
    ] {
        let reply = harness.send(get(path)).await;
        assert_eq!(reply.status, expected_status, "{path}");
        let csp = reply.header("content-security-policy");
        assert!(csp.contains("default-src 'self'"), "{path}: {csp}");
        assert!(
            csp.contains("connect-src 'self' wss://tandem.test"),
            "{csp}"
        );
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"));
        assert_eq!(
            reply.header("strict-transport-security"),
            "max-age=31536000; includeSubDomains"
        );
        assert_eq!(reply.header("x-content-type-options"), "nosniff");
        assert_eq!(reply.header("x-frame-options"), "DENY");
        assert_eq!(
            reply.header("referrer-policy"),
            "strict-origin-when-cross-origin"
        );
    }
    assert_eq!(
        harness.send(get("/healthz")).await.header("cache-control"),
        "no-store"
    );
}

#[tokio::test]
async fn cors_is_never_enabled() {
    let harness = Harness::new();
    let reply = harness
        .send(get("/api/setup").header("origin", "https://evil.test"))
        .await;
    assert!(reply.headers.get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn a_request_id_is_generated_and_incoming_ones_are_ignored_without_a_trusted_proxy() {
    let harness = Harness::new();
    let reply = harness
        .send(get("/healthz").header("x-request-id", "attacker-chosen-id-0001"))
        .await;
    let id = reply.header("x-request-id");
    assert_eq!(id.len(), 32);
    assert_ne!(id, "attacker-chosen-id-0001");
    let again = harness.send(get("/healthz")).await.header("x-request-id");
    assert_ne!(id, again);
}

#[tokio::test]
async fn a_trusted_proxy_may_supply_the_request_id_but_only_if_it_is_well_formed() {
    let harness = Harness::with(Options {
        trusted_proxies: Some("10.0.0.0/8"),
        ..Options::default()
    });
    let reply = harness
        .send(
            get("/healthz")
                .peer("10.1.2.3:5000")
                .header("x-request-id", "proxy-id-0001"),
        )
        .await;
    assert_eq!(reply.header("x-request-id"), "proxy-id-0001");
    let bad = harness
        .send(
            get("/healthz")
                .peer("10.1.2.3:5000")
                .header("x-request-id", "bad id!"),
        )
        .await;
    assert_ne!(bad.header("x-request-id"), "bad id!");
    let untrusted = harness
        .send(
            get("/healthz")
                .peer("203.0.113.9:5000")
                .header("x-request-id", "proxy-id-0001"),
        )
        .await;
    assert_ne!(untrusted.header("x-request-id"), "proxy-id-0001");
}

// ---- errors, validation and fail-closed behaviour -----------------------------------------

#[tokio::test]
async fn a_panic_becomes_a_generic_500_without_the_message() {
    let router: Router = Router::new()
        .route("/boom", axum_get(boom))
        .layer(CatchPanicLayer::custom(catch_panic_response));
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/boom")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = axum::body::to_bytes(response.into_body(), 10_000)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("\"code\":\"internal\""));
    assert!(!text.contains("secret internal detail"));
}

#[tokio::test]
async fn unknown_api_paths_are_json_not_found() {
    let harness = Harness::new();
    let reply = harness.send(get("/api/definitely/not/real")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "not_found");
}

#[tokio::test]
async fn bad_bodies_use_the_error_envelope() {
    let harness = Harness::new();
    let malformed = harness
        .send(
            post("/api/setup")
                .origin()
                .raw_body("application/json", "{not json"),
        )
        .await;
    assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
    assert_eq!(malformed.error_code(), "bad_request");

    let wrong_type = harness
        .send(post("/api/setup").origin().raw_body("text/plain", "hello"))
        .await;
    assert_eq!(wrong_type.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let unknown_field = harness
        .send(post("/api/setup").origin().json(&json!({
            "display_name": "A", "email": "a@example.test", "password": PASSWORD, "role": "admin"
        })))
        .await;
    assert_eq!(
        unknown_field.status,
        StatusCode::BAD_REQUEST,
        "unknown fields are denied"
    );

    let invalid = harness
        .send(post("/api/setup").origin().json(&json!({
            "display_name": "", "email": "not-an-email", "password": "short"
        })))
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body = invalid.json();
    assert_eq!(body["error"]["code"], "validation");
    assert!(!body["error"]["fields"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn oversized_bodies_are_refused_before_parsing() {
    let harness = Harness::new();
    let huge = "x".repeat(1_100_000);
    let reply = harness
        .send(
            post("/api/auth/sign-in")
                .origin()
                .raw_body("application/json", &huge),
        )
        .await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.error_code(), "payload_too_large");
}

// ---- rate limiting -------------------------------------------------------------------------

#[tokio::test]
async fn sign_in_is_limited_per_source_with_retry_after() {
    let harness = Harness::new();
    let attempt = |peer: &'static str, index: usize| {
        post("/api/auth/sign-in")
            .origin()
            .peer(peer)
            .json(&json!({"email": format!("nobody{index}@example.test"), "password": "wrong"}))
    };
    for index in 0..20 {
        let reply = harness.send(attempt("198.51.100.20:1", index)).await;
        assert_ne!(reply.status, StatusCode::TOO_MANY_REQUESTS);
    }
    let limited = harness.send(attempt("198.51.100.20:1", 99)).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.error_code(), "rate_limited");
    assert!(limited.header("retry-after").parse::<u64>().unwrap() >= 1);
    let other_source = harness.send(attempt("198.51.100.21:1", 98)).await;
    assert_ne!(other_source.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn forwarded_headers_cannot_dodge_the_limit_unless_the_peer_is_a_trusted_proxy() {
    let harness = Harness::new();
    let attempt = |forwarded: &str, index: usize| {
        post("/api/auth/sign-in")
            .origin()
            .peer("198.51.100.30:1")
            .header("x-forwarded-for", forwarded)
            .json(&json!({"email": format!("nobody{index}@example.test"), "password": "wrong"}))
    };
    for index in 0..20 {
        harness
            .send(attempt(&format!("203.0.113.{index}"), index))
            .await;
    }
    assert_eq!(
        harness.send(attempt("203.0.113.200", 200)).await.status,
        StatusCode::TOO_MANY_REQUESTS,
        "a spoofed X-Forwarded-For from an untrusted peer changes nothing"
    );

    let proxied = Harness::with(Options {
        trusted_proxies: Some("198.51.100.0/24"),
        ..Options::default()
    });
    for index in 0..20 {
        proxied
            .send(attempt(&format!("203.0.113.{index}"), index))
            .await;
    }
    // Each distinct client behind the trusted proxy has its own allowance.
    let fresh = proxied.send(attempt("203.0.113.201", 201)).await;
    assert_ne!(fresh.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn first_run_setup_has_the_tightest_limit() {
    let harness = Harness::new();
    let attempt = || {
        post("/api/setup")
            .origin()
            .peer("198.51.100.40:1")
            .json(&json!({"display_name": "", "email": "x", "password": "y"}))
    };
    for _ in 0..10 {
        assert_ne!(
            harness.send(attempt()).await.status,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    assert_eq!(
        harness.send(attempt()).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

// ---- sessions, CSRF and Origin -------------------------------------------------------------

#[tokio::test]
async fn the_session_cookie_has_the_required_attributes_and_sign_out_ends_it() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let signed_in = harness.sign_in("ada@example.test", PASSWORD).await;
    assert_eq!(signed_in.json()["status"], "mfa_required");

    let setup_reply_cookie_line = {
        let harness = Harness::new();
        let reply = harness
            .send(post("/api/setup").origin().json(&json!({
                "display_name": "Ada", "email": "a@example.test", "password": PASSWORD
            })))
            .await;
        reply
            .headers
            .get_all("set-cookie")
            .iter()
            .map(|value| value.to_str().unwrap().to_owned())
            .next()
            .unwrap()
    };
    assert!(setup_reply_cookie_line.starts_with("__Host-tandem_session="));
    for attribute in ["Path=/", "Secure", "HttpOnly", "SameSite=Lax"] {
        assert!(
            setup_reply_cookie_line.contains(attribute),
            "{setup_reply_cookie_line}"
        );
    }
    assert!(!setup_reply_cookie_line.contains("Domain"));

    let out = harness
        .send(post("/api/auth/sign-out").signed_in(&admin))
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out.header("set-cookie").contains("Max-Age=0"));
    let after = harness
        .send(get("/api/auth/me").cookie(&admin.cookie))
        .await;
    assert_eq!(
        after.status,
        StatusCode::UNAUTHORIZED,
        "the ended session no longer works"
    );
}

#[tokio::test]
async fn unauthenticated_and_tampered_sessions_are_refused() {
    let harness = Harness::new();
    assert_eq!(
        harness.send(get("/api/auth/me")).await.status,
        StatusCode::UNAUTHORIZED
    );
    let tampered = harness
        .send(get("/api/auth/me").cookie("__Host-tandem_session=not-a-real-token"))
        .await;
    assert_eq!(tampered.status, StatusCode::UNAUTHORIZED);
    let wrong_name = harness
        .send(get("/api/auth/me").cookie("tandem_session=anything"))
        .await;
    assert_eq!(wrong_name.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn state_changing_calls_need_the_csrf_token_and_a_matching_origin() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let body = json!({"title": "Hello"});

    let no_csrf = harness
        .send(
            post("/api/chat/sessions")
                .cookie(&admin.cookie)
                .origin()
                .json(&body),
        )
        .await;
    assert_eq!(
        (no_csrf.status, no_csrf.error_code().as_str()),
        (StatusCode::FORBIDDEN, "csrf")
    );

    let wrong_csrf = harness
        .send(
            post("/api/chat/sessions")
                .cookie(&admin.cookie)
                .csrf("forged")
                .origin()
                .json(&body),
        )
        .await;
    assert_eq!(wrong_csrf.error_code(), "csrf");

    let no_origin = harness
        .send(
            post("/api/chat/sessions")
                .cookie(&admin.cookie)
                .csrf(&admin.csrf)
                .json(&body),
        )
        .await;
    assert_eq!(no_origin.error_code(), "missing_origin");

    let bad_origin = harness
        .send(
            post("/api/chat/sessions")
                .cookie(&admin.cookie)
                .csrf(&admin.csrf)
                .header("origin", "https://evil.test")
                .json(&body),
        )
        .await;
    assert_eq!(bad_origin.error_code(), "bad_origin");

    let ok = harness
        .send(post("/api/chat/sessions").signed_in(&admin).json(&body))
        .await;
    assert_eq!(ok.status, StatusCode::CREATED, "{}", ok.text);

    let other_users_token = harness
        .send(
            post("/api/chat/sessions")
                .cookie(&admin.cookie)
                .csrf(
                    &common::User {
                        cookie: String::new(),
                        csrf: "x".repeat(43),
                        id: String::new(),
                    }
                    .csrf,
                )
                .origin()
                .json(&body),
        )
        .await;
    assert_eq!(other_users_token.error_code(), "csrf");

    let read = harness
        .send(get("/api/chat/sessions").cookie(&admin.cookie))
        .await;
    assert_eq!(read.status, StatusCode::OK, "safe methods need no token");
}

#[tokio::test]
async fn a_public_state_changing_route_still_refuses_a_foreign_origin() {
    let harness = Harness::new();
    let reply = harness
        .send(
            post("/api/auth/sign-in")
                .header("origin", "https://evil.test")
                .json(&json!({"email": "a@example.test", "password": PASSWORD})),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.error_code(), "bad_origin");
}

// ---- authorisation ------------------------------------------------------------------------

#[tokio::test]
async fn admin_routes_are_hidden_from_members_and_anonymous_callers_are_asked_to_sign_in() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;

    let anonymous = harness.send(get("/api/admin/users")).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let as_member = harness
        .send(get("/api/admin/users").cookie(&member.cookie))
        .await;
    assert_eq!(as_member.status, StatusCode::NOT_FOUND, "BR-9.2");
    let as_admin = harness
        .send(get("/api/admin/users").cookie(&admin.cookie))
        .await;
    assert_eq!(as_admin.status, StatusCode::OK);
    assert_eq!(as_admin.json().as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn an_admin_who_has_not_enrolled_mfa_can_only_enrol() {
    let harness = Harness::new();
    let reply = harness
        .send(post("/api/setup").origin().json(&json!({
            "display_name": "Ada", "email": "ada@example.test", "password": PASSWORD
        })))
        .await;
    let user = harness.identify(&reply.session_cookie().unwrap()).await;
    for path in ["/api/chat/sessions", "/api/admin/users"] {
        let blocked = harness.send(get(path).cookie(&user.cookie)).await;
        assert_eq!(blocked.status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(blocked.error_code(), "mfa_enrolment_required");
    }
    let me = harness
        .send(get("/api/auth/me").cookie(&user.cookie))
        .await
        .json();
    assert_eq!(me["scope"], "mfa_enrolment_only");
}

#[tokio::test]
async fn a_demotion_takes_effect_on_the_very_next_request() {
    let harness = Harness::new();
    let first = harness.setup_admin("ada@example.test").await;
    let second_member = harness.add_member(&first, "bo@example.test").await;
    let promote = harness
        .send(
            put(&format!("/api/admin/users/{}/role", second_member.id))
                .signed_in(&first)
                .json(&json!({"role": "admin"})),
        )
        .await;
    assert_eq!(promote.status, StatusCode::NO_CONTENT, "{}", promote.text);
    // A new admin without MFA is enrolment-only, so enrol to get full scope.
    harness
        .send(post("/api/auth/mfa/enrol").signed_in(&second_member))
        .await;
    let code = harness.t.current_code(second_member.id.parse().unwrap());
    let confirmed = harness
        .send(
            post("/api/auth/mfa/confirm")
                .signed_in(&second_member)
                .json(&json!({"code": code})),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.text);
    assert_eq!(
        harness
            .send(get("/api/admin/users").cookie(&second_member.cookie))
            .await
            .status,
        StatusCode::OK
    );
    let demote = harness
        .send(
            put(&format!("/api/admin/users/{}/role", second_member.id))
                .signed_in(&first)
                .json(&json!({"role": "member"})),
        )
        .await;
    assert_eq!(demote.status, StatusCode::NO_CONTENT, "{}", demote.text);
    assert_eq!(
        harness
            .send(get("/api/admin/users").cookie(&second_member.cookie))
            .await
            .status,
        StatusCode::NOT_FOUND,
        "no cached role"
    );
}

#[tokio::test]
async fn the_last_admin_cannot_be_removed() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let demote = harness
        .send(
            put(&format!("/api/admin/users/{}/role", admin.id))
                .signed_in(&admin)
                .json(&json!({"role": "member"})),
        )
        .await;
    assert_eq!(demote.status, StatusCode::UNPROCESSABLE_ENTITY);
    let deactivate = harness
        .send(post(&format!("/api/admin/users/{}/deactivate", admin.id)).signed_in(&admin))
        .await;
    assert_eq!(deactivate.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_deactivated_user_is_signed_out_at_once() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;
    harness
        .send(post(&format!("/api/admin/users/{}/deactivate", member.id)).signed_in(&admin))
        .await;
    let after = harness
        .send(get("/api/auth/me").cookie(&member.cookie))
        .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED);
}

// ---- misuse cases (NFR requirements 5.1) -------------------------------------------------

#[tokio::test]
async fn unknown_and_known_accounts_get_identical_failures() {
    let harness = Harness::new();
    harness.setup_admin("ada@example.test").await;
    let wrong_password = harness
        .sign_in("ada@example.test", "Wrong-Password-1")
        .await;
    let unknown_email = harness
        .sign_in("ghost@example.test", "Wrong-Password-1")
        .await;
    assert_eq!(wrong_password.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password.status, unknown_email.status);
    assert_eq!(wrong_password.text, unknown_email.text);
}

#[tokio::test]
async fn repeated_failures_lock_the_account_and_the_lock_is_a_429() {
    let harness = Harness::new();
    harness.setup_admin("ada@example.test").await;
    let mut saw_refusal = false;
    for index in 0..12 {
        let reply = harness
            .send(
                post("/api/auth/sign-in")
                    .origin()
                    .peer(&format!("198.51.100.{}:1", 100 + index))
                    .json(&json!({"email": "ada@example.test", "password": "Wrong-Password-1"})),
            )
            .await;
        if reply.status == StatusCode::TOO_MANY_REQUESTS {
            assert!(reply.header("retry-after").parse::<u64>().is_ok());
            saw_refusal = true;
        }
    }
    assert!(
        saw_refusal,
        "the delay and lockout of BR-3 reach the client"
    );
    let right_password = harness.sign_in("ada@example.test", PASSWORD).await;
    assert_ne!(
        right_password.status,
        StatusCode::OK,
        "a locked account refuses even the right password"
    );
}

#[tokio::test]
async fn a_totp_code_cannot_be_replayed() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    harness.t.clock.advance(time::Duration::seconds(90));
    let code = harness.t.current_code(admin.id.parse().unwrap());
    let attempt = || {
        post("/api/auth/sign-in").origin().json(&json!({
            "email": "ada@example.test",
            "password": PASSWORD,
            "second_factor": {"kind": "totp", "code": code}
        }))
    };
    assert_eq!(harness.send(attempt()).await.status, StatusCode::OK);
    assert_eq!(
        harness.send(attempt()).await.status,
        StatusCode::UNAUTHORIZED,
        "the same time step is refused (BR-4.4)"
    );
}

#[tokio::test]
async fn an_invitation_link_works_once() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let invite = harness
        .send(
            post("/api/admin/invitations")
                .signed_in(&admin)
                .json(&json!({"email": "mia@example.test", "role": "member"})),
        )
        .await;
    let link = invite.json()["link"].as_str().unwrap().to_owned();
    let token = link.split("token=").nth(1).unwrap().to_owned();
    let accept = |password: &str| {
        post("/api/invitations/accept").origin().json(&json!({
            "token": token, "display_name": "Mia", "password": password
        }))
    };
    let valid = harness
        .send(
            post("/api/invitations/validate")
                .origin()
                .json(&json!({"token": token})),
        )
        .await;
    assert_eq!(valid.json()["email"], "mia@example.test");
    assert_eq!(harness.send(accept(PASSWORD)).await.status, StatusCode::OK);
    let second = harness.send(accept(PASSWORD)).await;
    assert_eq!(second.status, StatusCode::NOT_FOUND);
    assert_eq!(second.error_code(), "invalid_link");
    let guessed = harness
        .send(
            post("/api/invitations/validate")
                .origin()
                .json(&json!({"token": "guess"})),
        )
        .await;
    assert_eq!(
        guessed.text, second.text,
        "one generic answer for every bad link"
    );
}

#[tokio::test]
async fn a_reset_link_works_once_and_signs_the_user_out_everywhere() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;
    let issued = harness
        .send(
            post(&format!("/api/admin/users/{}/reset-link", member.id))
                .signed_in(&admin)
                .json(&json!({"reset_mfa": false})),
        )
        .await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.text);
    let token = issued.json()["link"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let complete = |password: &str| {
        post("/api/password-resets/complete")
            .origin()
            .json(&json!({"token": token, "new_password": password}))
    };
    assert_eq!(
        harness
            .send(complete("Brand-New-Passphrase-9"))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.send(complete("Another-Passphrase-10")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        harness
            .send(get("/api/auth/me").cookie(&member.cookie))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        harness
            .sign_in("mia@example.test", "Brand-New-Passphrase-9")
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn members_cannot_read_each_others_chats() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;
    let session = harness
        .send(
            post("/api/chat/sessions")
                .signed_in(&member)
                .json(&json!({"title": "Private"})),
        )
        .await
        .json();
    let id = session["id"].as_str().unwrap();
    let theirs = harness
        .send(get(&format!("/api/chat/sessions/{id}/messages")).cookie(&admin.cookie))
        .await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND);
    let post_into = harness
        .send(
            post(&format!("/api/chat/sessions/{id}/messages"))
                .signed_in(&admin)
                .json(&json!({"body": "hi"})),
        )
        .await;
    assert_eq!(post_into.status, StatusCode::NOT_FOUND);
    let malformed = harness
        .send(get("/api/chat/sessions/not-an-id/messages").cookie(&member.cookie))
        .await;
    assert_eq!(malformed.status, StatusCode::NOT_FOUND);
}

// ---- health and metrics -------------------------------------------------------------------

#[tokio::test]
async fn healthz_is_up_and_readyz_follows_the_probes_without_leaking_detail() {
    let harness = Harness::new();
    assert_eq!(harness.send(get("/healthz")).await.status, StatusCode::OK);
    let ready = harness.send(get("/readyz")).await;
    assert_eq!(ready.status, StatusCode::OK);
    assert_eq!(ready.json()["ready"], true);

    harness.database_ok.store(false, Ordering::SeqCst);
    let down = harness.send(get("/readyz")).await;
    assert_eq!(down.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!down.text.contains("secret-detail-that-must-not-leak"));
    assert_eq!(down.json()["checks"][0]["ok"], false);
    harness.database_ok.store(true, Ordering::SeqCst);

    harness.state.readiness.begin_shutdown();
    let stopping = harness.send(get("/readyz")).await;
    assert_eq!(stopping.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(harness.send(get("/healthz")).await.status, StatusCode::OK);
}

#[tokio::test]
async fn metrics_are_off_without_a_token_and_need_the_exact_bearer_token() {
    let off = Harness::new();
    assert_eq!(
        off.send(get("/metrics")).await.status,
        StatusCode::NOT_FOUND
    );

    let on = Harness::with(Options {
        metrics_token: Some("a-long-enough-metrics-token"),
        ..Options::default()
    });
    assert_eq!(
        on.send(get("/metrics")).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        on.send(get("/metrics").header("authorization", "Bearer wrong-token-value-here"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        on.send(get("/metrics").header("authorization", "a-long-enough-metrics-token"))
            .await
            .status,
        StatusCode::UNAUTHORIZED,
        "the Bearer scheme is required"
    );
    on.send(get("/api/auth/me")).await;
    let reply = on
        .send(get("/metrics").header("authorization", "Bearer a-long-enough-metrics-token"))
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    for name in [
        "http_requests_total",
        "http_request_duration_seconds",
        "auth_sign_in_total",
        "rate_limited_total",
        "db_pool_connections",
        "process_resident_memory_bytes",
        "build_info",
    ] {
        assert!(reply.text.contains(name), "{name} missing");
    }
    assert!(
        reply.text.contains("route=\"/api/auth/me\""),
        "route templates, not raw paths"
    );
}

// ---- start-up guard and contract ---------------------------------------------------------

#[tokio::test]
async fn every_route_is_public_or_guarded_and_the_public_list_is_deliberate() {
    let (_, info) = tandem_api::routes::all_routes().unwrap();
    let mut public: Vec<String> = info
        .iter()
        .filter(|(_, route)| route.access == tandem_api::pipeline::access::AccessKind::Public)
        .map(|((method, path), _)| format!("{method} {path}"))
        .collect();
    public.sort();
    assert_eq!(
        public,
        vec![
            "GET /api/about",
            "GET /api/openapi.json",
            "GET /api/setup",
            "GET /healthz",
            "GET /metrics",
            "GET /readyz",
            "POST /api/auth/sign-in",
            "POST /api/invitations/accept",
            "POST /api/invitations/validate",
            "POST /api/password-resets/complete",
            "POST /api/password-resets/validate",
            "POST /api/setup",
        ],
        "adding a public route must be a conscious change to this list"
    );
    assert!(
        info.keys()
            .filter(|(_, path)| path.starts_with("/api/admin"))
            .all(|(_, path)| {
                info[&(
                    info.keys()
                        .find(|(_, other)| other == path)
                        .unwrap()
                        .0
                        .clone(),
                    path.clone(),
                )]
                    .access
                    == tandem_api::pipeline::access::AccessKind::AdminUser
            })
    );
}

#[tokio::test]
async fn registering_a_route_twice_fails_the_start_up_check() {
    use axum::http::Method;
    use tandem_api::pipeline::rate_limit::Bucket;
    use tandem_api::routes::RouteTable;
    let result = RouteTable::new()
        .public(Method::GET, "/a", Bucket::General, || async { "a" })
        .public(Method::GET, "/a", Bucket::General, || async { "b" })
        .finish();
    assert!(result.is_err());
}

#[tokio::test]
async fn deleting_with_the_wrong_method_is_refused_not_routed_elsewhere() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let reply = harness
        .send(delete("/api/admin/settings").signed_in(&admin))
        .await;
    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
}
