use super::*;
use tandem_core::session::{EndReason, Scope};
use tandem_core::settings::SecuritySettings;
use time::Duration;

const MEMBER: &str = "mia@example.com";

async fn setup() -> (TestApp, RequestContext, String) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    let (_, token) = member(&t, &admin, MEMBER).await;
    (t, admin, token)
}

fn session_reason(t: &TestApp, token: &str) -> Option<EndReason> {
    let hash = tandem_core::token::hash_token(token);
    t.store.read(|s| {
        s.sessions
            .iter()
            .find(|session| session.token_hash == hash)
            .and_then(|session| session.ended_reason)
    })
}

#[tokio::test]
async fn an_unknown_token_is_unauthenticated() {
    let t = TestApp::new();
    assert!(matches!(
        t.app.resolve_session("nope").await,
        Err(AppError::Unauthenticated)
    ));
}

#[tokio::test]
async fn br_8_2_an_idle_sign_in_ends_after_the_idle_timeout() {
    let (t, _, token) = setup().await;
    t.clock.advance(Duration::hours(12) + Duration::seconds(1));
    assert!(matches!(
        t.app.resolve_session(&token).await,
        Err(AppError::Unauthenticated)
    ));
    assert_eq!(session_reason(&t, &token), Some(EndReason::IdleTimeout));
}

#[tokio::test]
async fn br_8_2_an_active_sign_in_ends_at_the_absolute_duration() {
    let (t, _, token) = setup().await;
    let start = t.clock.now_for_assert();
    let mut ended_after = None;
    for _ in 0..40 {
        t.clock.advance(Duration::hours(11));
        if t.app.resolve_session(&token).await.is_err() {
            ended_after = Some(t.clock.now_for_assert() - start);
            break;
        }
    }
    let elapsed = ended_after.expect("the sign-in must end");
    assert!(elapsed > Duration::days(14) && elapsed <= Duration::days(14) + Duration::hours(11));
    assert_eq!(session_reason(&t, &token), Some(EndReason::AbsoluteTimeout));
}

#[tokio::test]
async fn br_8_2_changed_durations_apply_only_to_new_sign_ins() {
    let (t, admin, old_token) = setup().await;
    let shorter = SecuritySettings {
        idle_timeout: Duration::minutes(15),
        ..SecuritySettings::default()
    };
    t.app
        .update_security_settings(&admin, shorter)
        .await
        .unwrap();
    let new_token = sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await.token;
    t.clock.advance(Duration::minutes(16));
    assert!(
        t.app.resolve_session(&new_token).await.is_err(),
        "new sign-in uses 15 minutes"
    );
    assert!(
        t.app.resolve_session(&old_token).await.is_ok(),
        "existing sign-in keeps 12 hours"
    );
}

#[tokio::test]
async fn last_seen_is_written_at_most_once_a_minute() {
    let (t, _, token) = setup().await;
    let hash = tandem_core::token::hash_token(&token);
    let seen = |t: &TestApp| {
        t.store
            .read(|s| {
                s.sessions
                    .iter()
                    .find(|x| x.token_hash == hash)
                    .map(|x| x.last_seen_at)
            })
            .unwrap()
    };
    let first = seen(&t);
    t.clock.advance(Duration::seconds(30));
    t.app.resolve_session(&token).await.unwrap();
    assert_eq!(seen(&t), first, "within a minute nothing is written");
    t.clock.advance(Duration::seconds(30));
    t.app.resolve_session(&token).await.unwrap();
    assert_eq!(
        seen(&t),
        t.clock.now_for_assert(),
        "after a minute it is written"
    );
}

#[tokio::test]
async fn br_9_1_and_8_4_deactivation_takes_effect_on_the_very_next_request() {
    let (t, admin, token) = setup().await;
    t.app
        .deactivate_user(&admin, user_id(&t, MEMBER))
        .await
        .unwrap();
    assert!(matches!(
        t.app.resolve_session(&token).await,
        Err(AppError::Unauthenticated)
    ));
    assert_eq!(session_reason(&t, &token), Some(EndReason::Deactivated));
}

#[tokio::test]
async fn br_9_1_a_demotion_changes_the_role_on_the_next_request() {
    let (t, admin, _) = setup().await;
    let second = t
        .app
        .invite(&admin, "bo@example.com", Role::Admin)
        .await
        .unwrap();
    let new = t
        .app
        .accept_invitation(&t.anonymous(), &second.token, "Bo Admin", GOOD_PASSWORD)
        .await
        .unwrap();
    assert_eq!(
        t.app.resolve_session(&new.token).await.unwrap().actor.role,
        Role::Admin
    );
    t.app
        .change_user_role(&admin, user_id(&t, "bo@example.com"), Role::Member)
        .await
        .unwrap();
    assert_eq!(
        t.app.resolve_session(&new.token).await.unwrap().actor.role,
        Role::Member
    );
}

#[tokio::test]
async fn br_4_1_a_member_promoted_to_admin_is_limited_to_enrolment_until_they_enrol() {
    let (t, admin, token) = setup().await;
    assert_eq!(
        t.app.resolve_session(&token).await.unwrap().actor.scope,
        Scope::Full
    );
    t.app
        .change_user_role(&admin, user_id(&t, MEMBER), Role::Admin)
        .await
        .unwrap();
    let limited = t.context_for(&token).await;
    assert_eq!(limited.actor().unwrap().scope, Scope::MfaEnrolmentOnly);
    assert!(matches!(
        t.app.list_users(&limited).await,
        Err(AppError::Forbidden)
    ));
    t.enrol_mfa(&limited).await;
    assert_eq!(
        t.app.resolve_session(&token).await.unwrap().actor.scope,
        Scope::Full
    );
}

#[tokio::test]
async fn br_8_1_sign_out_ends_only_that_sign_in_and_is_audited() {
    let (t, _, token) = setup().await;
    let other = sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await.token;
    let ctx = t.context_for(&token).await;
    t.app.sign_out(&ctx).await.unwrap();
    assert!(t.app.resolve_session(&token).await.is_err());
    assert!(
        t.app.resolve_session(&other).await.is_ok(),
        "BR-8.5: several sign-ins can coexist"
    );
    assert_eq!(count_audit(&t, "auth.sign_out"), 1);
}

#[tokio::test]
async fn the_current_user_reports_scope_and_mfa_state() {
    let (t, _, token) = setup().await;
    let me = t
        .app
        .current_user(&t.context_for(&token).await)
        .await
        .unwrap();
    assert_eq!(me.user.email.as_str(), MEMBER);
    assert!(!me.mfa.enrolled);
    assert_eq!(me.scope, Scope::Full);
}
