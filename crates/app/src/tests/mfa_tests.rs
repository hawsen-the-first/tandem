use super::*;
use std::collections::HashSet;
use tandem_core::session::Scope;

const MEMBER: &str = "mia@example.com";

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    let (ctx, _) = member(&t, &admin, MEMBER).await;
    (t, ctx)
}

#[tokio::test]
async fn br_4_5_enrolment_yields_ten_distinct_recovery_codes_and_is_audited() {
    let (t, ctx) = setup().await;
    let codes = t.enrol_mfa(&ctx).await;
    assert_eq!(codes.len(), 10);
    assert_eq!(codes.iter().collect::<HashSet<_>>().len(), 10);
    assert_eq!(
        count_audit(&t, "auth.mfa.enrolled"),
        2,
        "the admin and the member enrolled"
    );
    let status = t.app.mfa_status(&ctx).await.unwrap();
    assert!(status.enrolled && status.recovery_codes_remaining == 10 && !status.remind_regenerate);
}

#[tokio::test]
async fn br_4_1_confirming_enrolment_upgrades_an_enrolment_only_sign_in() {
    let t = TestApp::new();
    let token = t.setup_admin("ada@example.com").await;
    let ctx = t.context_for(&token).await;
    assert_eq!(ctx.actor().unwrap().scope, Scope::MfaEnrolmentOnly);
    t.enrol_mfa(&ctx).await;
    assert_eq!(
        t.app.resolve_session(&token).await.unwrap().actor.scope,
        Scope::Full
    );
}

#[tokio::test]
async fn br_4_1_an_enrolment_only_sign_in_cannot_do_anything_else() {
    let t = TestApp::new();
    let token = t.setup_admin("ada@example.com").await;
    let ctx = t.context_for(&token).await;
    assert!(matches!(
        t.app
            .change_password(&ctx, GOOD_PASSWORD, "New-Password-9")
            .await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        t.app.create_chat_session(&ctx, "x").await,
        Err(AppError::Forbidden)
    ));
    assert!(t.app.mfa_status(&ctx).await.is_ok());
    assert_eq!(count_audit(&t, "authz.denied"), 2);
}

#[tokio::test]
async fn a_wrong_code_does_not_confirm_enrolment() {
    let (t, ctx) = setup().await;
    t.app.mfa_start_enrolment(&ctx).await.unwrap();
    let result = t.app.mfa_confirm_enrolment(&ctx, "000000").await;
    assert!(matches!(result, Err(AppError::Validation(_))));
    assert!(!t.app.mfa_status(&ctx).await.unwrap().enrolled);
}

#[tokio::test]
async fn confirming_without_starting_is_refused_and_starting_twice_after_confirming_is_too() {
    let (t, ctx) = setup().await;
    assert!(matches!(
        t.app.mfa_confirm_enrolment(&ctx, "123456").await,
        Err(AppError::Validation(_))
    ));
    t.enrol_mfa(&ctx).await;
    assert!(matches!(
        t.app.mfa_start_enrolment(&ctx).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn br_2_secrets_are_stored_sealed_never_in_plain() {
    let (t, ctx) = setup().await;
    let enrolment = t.app.mfa_start_enrolment(&ctx).await.unwrap();
    assert!(
        enrolment
            .otpauth_uri
            .starts_with("otpauth://totp/Tandem:mia@example.com")
    );
    let sealed = t
        .store
        .read(|s| s.credentials.last().map(|c| c.secret_sealed.clone()))
        .unwrap();
    assert!(sealed.starts_with(b"sealed:mfa_secret:"));
}

#[tokio::test]
async fn br_4_5_regenerating_replaces_every_earlier_code() {
    let (t, ctx) = setup().await;
    let old = t.enrol_mfa(&ctx).await;
    let new = t.app.mfa_regenerate_recovery_codes(&ctx).await.unwrap();
    assert_eq!(new.len(), 10);
    assert!(old.iter().all(|code| !new.contains(code)));
    let hash = tandem_core::token::hash_recovery_code(&old[0]);
    assert!(
        t.store
            .read(|s| s.recovery_codes.iter().all(|c| c.code_hash != hash))
    );
    assert_eq!(count_audit(&t, "auth.mfa.recovery_regenerated"), 1);
}

#[tokio::test]
async fn regenerating_needs_enrolled_mfa() {
    let (t, ctx) = setup().await;
    assert!(matches!(
        t.app.mfa_regenerate_recovery_codes(&ctx).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn a_reminder_appears_when_three_or_fewer_recovery_codes_remain() {
    let (t, ctx) = setup().await;
    t.enrol_mfa(&ctx).await;
    let uid = ctx.actor().unwrap().user_id;
    t.store.write(|s| {
        for code in s
            .recovery_codes
            .iter_mut()
            .filter(|c| c.user_id == uid)
            .take(7)
        {
            code.used_at = Some(time::OffsetDateTime::UNIX_EPOCH);
        }
    });
    let status = t.app.mfa_status(&ctx).await.unwrap();
    assert_eq!(status.recovery_codes_remaining, 3);
    assert!(status.remind_regenerate);
}

#[tokio::test]
async fn br_4_6_a_member_can_switch_mfa_off_with_password_and_a_current_code() {
    let (t, ctx) = setup().await;
    t.enrol_mfa(&ctx).await;
    let uid = ctx.actor().unwrap().user_id;
    t.clock.advance(time::Duration::minutes(2));
    let wrong_password = t
        .app
        .mfa_disable(&ctx, "Nope-Nope-1", &t.current_code(uid))
        .await;
    assert!(matches!(wrong_password, Err(AppError::Validation(_))));
    let wrong_code = t.app.mfa_disable(&ctx, GOOD_PASSWORD, "000000").await;
    assert!(matches!(wrong_code, Err(AppError::Validation(_))));
    t.app
        .mfa_disable(&ctx, GOOD_PASSWORD, &t.current_code(uid))
        .await
        .unwrap();
    assert!(!t.app.mfa_status(&ctx).await.unwrap().enrolled);
    assert_eq!(count_audit(&t, "auth.mfa.disabled"), 1);
    assert!(
        t.store
            .read(|s| s.recovery_codes.iter().all(|c| c.user_id != uid))
    );
}

#[tokio::test]
async fn br_4_6_an_admin_cannot_switch_mfa_off() {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    t.clock.advance(time::Duration::minutes(2));
    let code = t.current_code(admin.actor().unwrap().user_id);
    assert!(matches!(
        t.app.mfa_disable(&admin, GOOD_PASSWORD, &code).await,
        Err(AppError::Forbidden)
    ));
}
