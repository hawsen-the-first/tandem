//! Business rules that had no test of their own at the HTTP level, named after their IDs so the
//! BR-to-test table can find them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Harness, PASSWORD, get, post};
use serde_json::json;

#[tokio::test]
async fn br_2_5_the_same_password_policy_applies_at_setup_invitation_change_and_reset() {
    let weak = "short";
    let harness = Harness::new();

    // Setup.
    let setup = harness
        .send(post("/api/setup").origin().json(&json!({
            "display_name": "Ada", "email": "ada@example.test", "password": weak
        })))
        .await;
    assert_eq!(setup.status, StatusCode::UNPROCESSABLE_ENTITY);

    let admin = harness.setup_admin("ada@example.test").await;

    // Invitation acceptance.
    let invite = harness
        .send(
            post("/api/admin/invitations")
                .signed_in(&admin)
                .json(&json!({"email": "mia@example.test", "role": "member"})),
        )
        .await
        .json();
    let token = invite["link"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let accept = harness
        .send(post("/api/invitations/accept").origin().json(&json!({
            "token": token, "display_name": "Mia", "password": weak
        })))
        .await;
    assert_eq!(accept.status, StatusCode::UNPROCESSABLE_ENTITY);
    // The link is still usable after the weak attempt, and a good password works.
    let member = harness.add_member(&admin, "bo@example.test").await;

    // Password change.
    let change = harness
        .send(
            post("/api/auth/password")
                .signed_in(&member)
                .json(&json!({"current_password": PASSWORD, "new_password": weak})),
        )
        .await;
    assert_eq!(change.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Reset completion.
    let issued = harness
        .send(
            post(&format!("/api/admin/users/{}/reset-link", member.id))
                .signed_in(&admin)
                .json(&json!({"reset_mfa": false})),
        )
        .await
        .json();
    let reset_token = issued["link"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let reset = harness
        .send(post("/api/password-resets/complete").origin().json(&json!({
            "token": reset_token, "new_password": weak
        })))
        .await;
    assert_eq!(reset.status, StatusCode::UNPROCESSABLE_ENTITY);
    for reply in [&setup, &accept, &change, &reset] {
        assert!(!reply.text.contains(weak), "the password is never echoed");
    }
}

#[tokio::test]
async fn br_3_7_and_6_4_there_is_no_unlock_and_no_self_service_forgot_password() {
    let (_, info) = tandem_api::routes::all_routes().unwrap();
    for (_, path) in info.keys() {
        let lowered = path.to_lowercase();
        assert!(
            !lowered.contains("unlock"),
            "{path}: admins cannot unlock accounts (BR-3.7)"
        );
        assert!(
            !lowered.contains("forgot"),
            "{path}: no self-service forgot-password (BR-6.4)"
        );
    }
    // The only way to start a reset is an admin-only route.
    let issuers: Vec<_> = info
        .iter()
        .filter(|((_, path), _)| path.contains("reset-link"))
        .map(|(_, route)| route.access)
        .collect();
    assert_eq!(
        issuers,
        vec![tandem_api::pipeline::access::AccessKind::AdminUser]
    );
}

#[tokio::test]
async fn br_4_2_mfa_is_optional_for_members_and_they_can_enrol_at_any_time() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;
    // Signed in and fully working without MFA.
    let me = harness
        .send(get("/api/auth/me").cookie(&member.cookie))
        .await
        .json();
    assert_eq!(me["scope"], "full");
    assert_eq!(me["mfa"]["enrolled"], false);
    assert_eq!(
        harness
            .send(get("/api/chat/sessions").cookie(&member.cookie))
            .await
            .status,
        StatusCode::OK
    );
    // ...and can enrol whenever they like.
    let enrol = harness
        .send(post("/api/auth/mfa/enrol").signed_in(&member))
        .await;
    assert_eq!(enrol.status, StatusCode::OK, "{}", enrol.text);
}

#[tokio::test]
async fn br_5_3_an_invitation_link_is_shown_once_and_cannot_be_listed_again() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let created = harness
        .send(
            post("/api/admin/invitations")
                .signed_in(&admin)
                .json(&json!({"email": "mia@example.test", "role": "member"})),
        )
        .await
        .json();
    let token = created["link"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap()
        .to_owned();
    let listed = harness
        .send(get("/api/admin/invitations").cookie(&admin.cookie))
        .await;
    assert!(
        !listed.text.contains(&token),
        "the list never carries the token"
    );
    assert!(
        !listed.text.contains("link"),
        "the list never carries a link"
    );
    assert!(!listed.text.contains("token"));
}

#[tokio::test]
async fn br_8_5_a_person_can_hold_several_sessions_at_once() {
    let harness = Harness::new();
    let admin = harness.setup_admin("ada@example.test").await;
    let member = harness.add_member(&admin, "mia@example.test").await;
    let second = harness.sign_in("mia@example.test", PASSWORD).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text);
    let second_cookie = second.session_cookie().unwrap();
    assert_ne!(second_cookie, member.cookie);
    for cookie in [&member.cookie, &second_cookie] {
        assert_eq!(
            harness
                .send(get("/api/auth/me").cookie(cookie))
                .await
                .status,
            StatusCode::OK
        );
    }
    // Signing out of one leaves the other.
    let second_user = harness.identify(&second_cookie).await;
    harness
        .send(post("/api/auth/sign-out").signed_in(&second_user))
        .await;
    assert_eq!(
        harness
            .send(get("/api/auth/me").cookie(&second_cookie))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        harness
            .send(get("/api/auth/me").cookie(&member.cookie))
            .await
            .status,
        StatusCode::OK
    );
}
