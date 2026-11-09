use super::*;
use tandem_core::session::{EndReason, Scope};
use time::Duration;

const MEMBER: &str = "mia@example.com";
const NEW_PASSWORD: &str = "Another-Good-9!";

async fn setup() -> (TestApp, RequestContext, RequestContext, String) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    let (ctx, token) = member(&t, &admin, MEMBER).await;
    (t, admin, ctx, token)
}

fn reason(t: &TestApp, token: &str) -> Option<EndReason> {
    let hash = tandem_core::token::hash_token(token);
    t.store.read(|s| {
        s.sessions
            .iter()
            .find(|x| x.token_hash == hash)
            .and_then(|x| x.ended_reason)
    })
}

#[tokio::test]
async fn br_8_3_changing_the_password_ends_other_sign_ins_but_not_this_one() {
    let (t, _, ctx, token) = setup().await;
    let other = sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await.token;
    t.app
        .change_password(&ctx, GOOD_PASSWORD, NEW_PASSWORD)
        .await
        .unwrap();
    assert!(t.app.resolve_session(&token).await.is_ok());
    assert!(t.app.resolve_session(&other).await.is_err());
    assert_eq!(reason(&t, &other), Some(EndReason::PasswordChanged));
    assert!(matches!(
        sign_in_err(&t, MEMBER, GOOD_PASSWORD).await,
        AppError::InvalidCredentials | AppError::Refused { .. }
    ));
    t.clock.advance(Duration::minutes(1));
    sign_in_ok(&t, MEMBER, NEW_PASSWORD).await;
    assert_eq!(count_audit(&t, "auth.password.changed"), 1);
}

#[tokio::test]
async fn br_2_4_the_new_password_must_differ_and_meet_the_policy() {
    let (t, _, ctx, _) = setup().await;
    let wrong_current = t
        .app
        .change_password(&ctx, "Not-The-One-1", NEW_PASSWORD)
        .await;
    assert!(
        matches!(wrong_current, Err(AppError::Validation(ref i)) if i[0].field == "current_password")
    );
    let same = t
        .app
        .change_password(&ctx, GOOD_PASSWORD, GOOD_PASSWORD)
        .await;
    assert!(matches!(same, Err(AppError::Validation(ref i)) if i[0].message.contains("different")));
    let weak = t.app.change_password(&ctx, GOOD_PASSWORD, "short").await;
    assert!(matches!(weak, Err(AppError::Validation(_))));
    t.breach.add(NEW_PASSWORD);
    let breached = t
        .app
        .change_password(&ctx, GOOD_PASSWORD, NEW_PASSWORD)
        .await;
    assert!(matches!(breached, Err(AppError::Validation(_))));
}

#[tokio::test]
async fn br_10_the_online_breach_check_follows_the_setting() {
    use std::sync::atomic::Ordering;
    let (t, admin, ctx, _) = setup().await;
    t.app
        .change_password(&ctx, GOOD_PASSWORD, NEW_PASSWORD)
        .await
        .unwrap();
    assert!(!t.breach.last_used_online.load(Ordering::SeqCst));
    let settings = tandem_core::settings::SecuritySettings {
        online_breach_check: true,
        ..Default::default()
    };
    t.app
        .update_security_settings(&admin, settings)
        .await
        .unwrap();
    t.app
        .change_password(&ctx, NEW_PASSWORD, "Third-Good-Pass-5!")
        .await
        .unwrap();
    assert!(t.breach.last_used_online.load(Ordering::SeqCst));
}

#[tokio::test]
async fn br_6_forced_reset_ends_the_users_sign_ins_and_the_link_works_once() {
    let (t, admin, _, token) = setup().await;
    let target = user_id(&t, MEMBER);
    let link = t.app.issue_reset_link(&admin, target, false).await.unwrap();
    assert_eq!(reason(&t, &token), Some(EndReason::ResetIssued), "BR-6.3");
    assert_eq!(
        t.app
            .validate_reset_link(&link.token)
            .await
            .unwrap()
            .display_name,
        "Mia Member"
    );
    t.app
        .complete_reset(&t.anonymous(), &link.token, NEW_PASSWORD)
        .await
        .unwrap();
    sign_in_ok(&t, MEMBER, NEW_PASSWORD).await;
    assert!(matches!(
        t.app
            .complete_reset(&t.anonymous(), &link.token, "Yet-Another-1!")
            .await,
        Err(AppError::InvalidLink)
    ));
    assert!(matches!(
        t.app.validate_reset_link(&link.token).await,
        Err(AppError::InvalidLink)
    ));
    assert_eq!(count_audit(&t, "auth.password.reset_issued"), 1);
    assert_eq!(count_audit(&t, "auth.password.reset_completed"), 1);
}

#[tokio::test]
async fn br_6_1_a_reset_link_expires_after_24_hours() {
    let (t, admin, _, _) = setup().await;
    let link = t
        .app
        .issue_reset_link(&admin, user_id(&t, MEMBER), false)
        .await
        .unwrap();
    t.clock.advance(Duration::hours(24));
    assert!(matches!(
        t.app
            .complete_reset(&t.anonymous(), &link.token, NEW_PASSWORD)
            .await,
        Err(AppError::InvalidLink)
    ));
}

#[tokio::test]
async fn br_6_3_issuing_a_new_link_invalidates_the_earlier_one() {
    let (t, admin, _, _) = setup().await;
    let target = user_id(&t, MEMBER);
    let first = t.app.issue_reset_link(&admin, target, false).await.unwrap();
    let second = t.app.issue_reset_link(&admin, target, false).await.unwrap();
    assert!(matches!(
        t.app.validate_reset_link(&first.token).await,
        Err(AppError::InvalidLink)
    ));
    assert!(t.app.validate_reset_link(&second.token).await.is_ok());
}

#[tokio::test]
async fn br_6_2_unknown_links_get_the_generic_answer() {
    let t = TestApp::new();
    assert!(matches!(
        t.app.validate_reset_link("nope").await,
        Err(AppError::InvalidLink)
    ));
    assert!(matches!(
        t.app
            .complete_reset(&t.anonymous(), "nope", NEW_PASSWORD)
            .await,
        Err(AppError::InvalidLink)
    ));
}

#[tokio::test]
async fn br_6_a_weak_new_password_leaves_the_link_usable() {
    let (t, admin, _, _) = setup().await;
    let link = t
        .app
        .issue_reset_link(&admin, user_id(&t, MEMBER), false)
        .await
        .unwrap();
    assert!(matches!(
        t.app
            .complete_reset(&t.anonymous(), &link.token, "weak")
            .await,
        Err(AppError::Validation(_))
    ));
    t.app
        .complete_reset(&t.anonymous(), &link.token, NEW_PASSWORD)
        .await
        .unwrap();
}

#[tokio::test]
async fn br_6_5_also_reset_mfa_clears_it_and_an_admin_must_enrol_again() {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    let second = t
        .app
        .invite(&admin, "bo@example.com", Role::Admin)
        .await
        .unwrap();
    let bo = t
        .app
        .accept_invitation(&t.anonymous(), &second.token, "Bo Admin", GOOD_PASSWORD)
        .await
        .unwrap();
    let bo_ctx = t.context_for(&bo.token).await;
    t.enrol_mfa(&bo_ctx).await;
    let bo_id = user_id(&t, "bo@example.com");
    let link = t.app.issue_reset_link(&admin, bo_id, true).await.unwrap();
    t.app
        .complete_reset(&t.anonymous(), &link.token, NEW_PASSWORD)
        .await
        .unwrap();
    assert!(
        t.store
            .read(|s| s.credentials.iter().all(|c| c.user_id != bo_id))
    );
    assert!(
        t.store
            .read(|s| s.recovery_codes.iter().all(|c| c.user_id != bo_id))
    );
    let again = sign_in_ok(&t, "bo@example.com", NEW_PASSWORD).await;
    assert_eq!(again.scope, Scope::MfaEnrolmentOnly);
}

#[tokio::test]
async fn br_6_without_the_tick_mfa_survives_a_reset() {
    let (t, admin, ctx, _) = setup().await;
    t.enrol_mfa(&ctx).await;
    let target = user_id(&t, MEMBER);
    let link = t.app.issue_reset_link(&admin, target, false).await.unwrap();
    t.app
        .complete_reset(&t.anonymous(), &link.token, NEW_PASSWORD)
        .await
        .unwrap();
    assert!(t.store.read(|s| {
        s.credentials
            .iter()
            .any(|c| c.user_id == target && c.is_confirmed())
    }));
}

#[tokio::test]
async fn br_9_2_only_admins_can_issue_reset_links() {
    let (t, _, ctx, _) = setup().await;
    let result = t
        .app
        .issue_reset_link(&ctx, user_id(&t, MEMBER), false)
        .await;
    assert!(matches!(result, Err(AppError::NotFound)));
    assert_eq!(count_audit(&t, "authz.denied"), 1);
}
