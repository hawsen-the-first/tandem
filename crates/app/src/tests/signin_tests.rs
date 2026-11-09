use super::*;
use crate::SecondFactor;
use std::sync::atomic::Ordering;
use tandem_core::session::Scope;
use tandem_core::throttle::GuardState;
use time::Duration;

const MEMBER: &str = "mia@example.com";

async fn with_member() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    member(&t, &admin, MEMBER).await;
    (t, admin)
}

#[tokio::test]
async fn br_1_2_a_member_signs_in_with_a_full_scope_and_it_is_audited() {
    let (t, _) = with_member().await;
    let session = sign_in_ok(&t, "  MIA@example.com ", GOOD_PASSWORD).await;
    assert_eq!(session.scope, Scope::Full);
    assert_eq!(count_audit(&t, "auth.sign_in.success"), 1);
    assert!(
        t.store
            .read(|s| s.users.iter().any(|u| u.last_sign_in_at.is_some()))
    );
}

#[tokio::test]
async fn br_3_5_a_wrong_password_and_an_unknown_email_get_the_same_answer() {
    let (t, _) = with_member().await;
    let wrong = sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
    let unknown = sign_in_err(&t, "nobody@example.com", "Wrong-Password-1").await;
    assert!(matches!(wrong, AppError::InvalidCredentials));
    assert!(matches!(unknown, AppError::InvalidCredentials));
    assert_eq!(
        t.hasher.dummy_calls.load(Ordering::SeqCst),
        1,
        "unknown accounts do the same work"
    );
}

#[tokio::test]
async fn br_11_3_failures_are_written_only_for_existing_accounts() {
    let (t, _) = with_member().await;
    sign_in_err(&t, "nobody@example.com", "Wrong-Password-1").await;
    sign_in_err(&t, "not an email", "Wrong-Password-1").await;
    assert_eq!(count_audit(&t, "auth.sign_in.failure"), 0);
    t.clock.advance(Duration::minutes(5));
    sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
    assert_eq!(count_audit(&t, "auth.sign_in.failure"), 1);
    let stored = t.store.read(|s| format!("{:?}", s.audit));
    assert!(
        !stored.contains("Wrong-Password-1"),
        "the attempted password is never stored"
    );
}

#[tokio::test]
async fn br_1_4_a_deactivated_account_is_indistinguishable_from_a_wrong_password() {
    let (t, admin) = with_member().await;
    t.app
        .deactivate_user(&admin, user_id(&t, MEMBER))
        .await
        .unwrap();
    let error = sign_in_err(&t, MEMBER, GOOD_PASSWORD).await;
    assert!(matches!(error, AppError::InvalidCredentials));
    assert_eq!(t.hasher.dummy_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn br_3_1_a_failure_delays_the_next_attempt_and_the_delay_ends() {
    let (t, _) = with_member().await;
    sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
    let error = sign_in_err(&t, MEMBER, GOOD_PASSWORD).await;
    let AppError::Refused { retry_at } = error else {
        panic!("expected a refusal, got {error:?}")
    };
    assert_eq!(retry_at, t.clock.now_for_assert() + Duration::seconds(1));
    t.clock.advance(Duration::seconds(1));
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
}

#[tokio::test]
async fn br_3_2_ten_failures_in_an_hour_lock_the_account_for_fifteen_minutes() {
    let (t, _) = with_member().await;
    for _ in 0..10 {
        sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
        t.clock.advance(Duration::seconds(61));
    }
    // Let the last delay lapse but not the lock; the lock began at the tenth failure.
    assert_eq!(count_audit(&t, "auth.sign_in.locked"), 1);
    let error = sign_in_err(&t, MEMBER, GOOD_PASSWORD).await;
    assert!(
        matches!(error, AppError::Refused { .. }),
        "even the right password is refused while locked"
    );
    t.clock.advance(Duration::minutes(15));
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
}

#[tokio::test]
async fn br_3_3_success_resets_the_failure_count() {
    let (t, _) = with_member().await;
    sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
    t.clock.advance(Duration::seconds(2));
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
    let uid = user_id(&t, MEMBER);
    let guard = t
        .store
        .read(|s| s.account_guards.get(&uid).cloned().unwrap());
    assert_eq!(guard, GuardState::default());
}

#[tokio::test]
async fn br_3_5_unknown_emails_are_throttled_exactly_like_known_ones() {
    let (t, _) = with_member().await;
    sign_in_err(&t, "ghost@example.com", "Wrong-Password-1").await;
    let error = sign_in_err(&t, "ghost@example.com", "Wrong-Password-1").await;
    assert!(matches!(error, AppError::Refused { .. }));
}

#[tokio::test]
async fn br_3_6_a_refused_attempt_does_not_check_the_password() {
    let (t, _) = with_member().await;
    sign_in_err(&t, MEMBER, "Wrong-Password-1").await;
    let before = t.hasher.verify_calls.load(Ordering::SeqCst);
    sign_in_err(&t, MEMBER, GOOD_PASSWORD).await;
    assert_eq!(t.hasher.verify_calls.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn nfr_design_4_a_busy_hasher_answers_busy_and_does_not_count_as_a_failure() {
    let (t, _) = with_member().await;
    t.hasher.busy.store(true, Ordering::SeqCst);
    let error = sign_in_err(&t, MEMBER, GOOD_PASSWORD).await;
    assert!(matches!(error, AppError::Busy));
    t.hasher.busy.store(false, Ordering::SeqCst);
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
    assert_eq!(count_audit(&t, "auth.sign_in.failure"), 0);
}

#[tokio::test]
async fn nfr_design_4_an_outdated_hash_is_replaced_on_a_successful_sign_in() {
    let (t, _) = with_member().await;
    t.store.write(|s| {
        for user in &mut s.users {
            if user.email.as_str() == MEMBER {
                user.password_hash = format!("fake$v1${GOOD_PASSWORD}");
            }
        }
    });
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
    let stored = t.store.read(|s| {
        s.users
            .iter()
            .find(|u| u.email.as_str() == MEMBER)
            .map(|u| u.password_hash.clone())
    });
    assert_eq!(stored.unwrap(), format!("fake$v2${GOOD_PASSWORD}"));
}

#[tokio::test]
async fn br_4_1_an_admin_without_mfa_gets_an_enrolment_only_sign_in() {
    let t = TestApp::new();
    t.setup_admin("ada@example.com").await;
    let session = sign_in_ok(&t, "ada@example.com", GOOD_PASSWORD).await;
    assert_eq!(session.scope, Scope::MfaEnrolmentOnly);
}

async fn member_with_mfa() -> (TestApp, Vec<String>) {
    let (t, _) = with_member().await;
    let session = sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
    let ctx = t.context_for(&session.token).await;
    let codes = t.enrol_mfa(&ctx).await;
    t.clock.advance(Duration::minutes(2));
    (t, codes)
}

#[tokio::test]
async fn br_4_3_with_mfa_the_password_alone_asks_for_a_second_factor_without_counting_a_failure() {
    let (t, _) = member_with_mfa().await;
    let outcome = t
        .app
        .sign_in(&t.anonymous(), credentials(MEMBER, GOOD_PASSWORD))
        .await
        .unwrap();
    assert!(matches!(outcome, SignInOutcome::MfaRequired));
    let uid = user_id(&t, MEMBER);
    assert_eq!(
        t.store
            .read(|s| s.account_guards.get(&uid).cloned().unwrap_or_default()),
        GuardState::default()
    );
}

#[tokio::test]
async fn br_4_3_and_4_8_a_correct_code_signs_in_and_a_wrong_code_counts_as_a_failure() {
    let (t, _) = member_with_mfa().await;
    let uid = user_id(&t, MEMBER);
    let mut input = credentials(MEMBER, GOOD_PASSWORD);
    input.second_factor = Some(SecondFactor::Totp("000000".to_owned()));
    let wrong = t.app.sign_in(&t.anonymous(), input).await;
    assert!(matches!(wrong, Err(AppError::InvalidCredentials)));
    assert_eq!(t.store.read(|s| s.account_guards[&uid].failure_count), 1);
    t.clock.advance(Duration::seconds(2));
    let mut input = credentials(MEMBER, GOOD_PASSWORD);
    input.second_factor = Some(SecondFactor::Totp(t.current_code(uid)));
    assert!(matches!(
        t.app.sign_in(&t.anonymous(), input).await,
        Ok(SignInOutcome::Success(_))
    ));
}

#[tokio::test]
async fn br_4_4_a_totp_code_cannot_be_replayed() {
    let (t, _) = member_with_mfa().await;
    let uid = user_id(&t, MEMBER);
    let code = t.current_code(uid);
    let mut input = credentials(MEMBER, GOOD_PASSWORD);
    input.second_factor = Some(SecondFactor::Totp(code.clone()));
    assert!(matches!(
        t.app.sign_in(&t.anonymous(), input.clone()).await,
        Ok(SignInOutcome::Success(_))
    ));
    t.clock.advance(Duration::seconds(2));
    assert!(matches!(
        t.app.sign_in(&t.anonymous(), input).await,
        Err(AppError::InvalidCredentials)
    ));
}

#[tokio::test]
async fn br_4_5_a_recovery_code_signs_in_once_and_is_audited() {
    let (t, codes) = member_with_mfa().await;
    let mut input = credentials(MEMBER, GOOD_PASSWORD);
    input.second_factor = Some(SecondFactor::Recovery(
        codes[0].to_uppercase().replace('-', " "),
    ));
    assert!(matches!(
        t.app.sign_in(&t.anonymous(), input.clone()).await,
        Ok(SignInOutcome::Success(_))
    ));
    assert_eq!(count_audit(&t, "auth.mfa.recovery_used"), 1);
    t.clock.advance(Duration::seconds(2));
    assert!(matches!(
        t.app.sign_in(&t.anonymous(), input).await,
        Err(AppError::InvalidCredentials)
    ));
}
