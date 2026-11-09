use super::*;
use crate::SetupInput;
use tandem_core::session::Scope;

fn input(email: &str, password: &str) -> SetupInput {
    SetupInput {
        display_name: "Ada Admin".to_owned(),
        email: email.to_owned(),
        password: password.to_owned(),
    }
}

#[tokio::test]
async fn br_1_1_setup_creates_an_admin_with_an_enrolment_only_sign_in_and_audits_it() {
    let t = TestApp::new();
    assert!(t.app.setup_available().await.unwrap());
    let session = t
        .app
        .first_run_setup(&t.anonymous(), input("ada@example.com", GOOD_PASSWORD))
        .await
        .unwrap();
    assert_eq!(session.user.role, Role::Admin);
    assert_eq!(session.scope, Scope::MfaEnrolmentOnly, "BR-4.1");
    assert!(!session.token.is_empty());
    assert_eq!(audit_types(&t), vec!["auth.setup"]);
    t.store.read(|s| {
        assert_eq!(s.users.len(), 1);
        assert_eq!(s.users[0].password_hash, format!("fake$v2${GOOD_PASSWORD}"));
        assert_eq!(
            s.sessions[0].token_hash,
            tandem_core::token::hash_token(&session.token)
        );
    });
}

#[tokio::test]
async fn br_1_1_setup_disappears_once_a_user_exists() {
    let t = TestApp::new();
    t.setup_admin("ada@example.com").await;
    assert!(!t.app.setup_available().await.unwrap());
    let result = t
        .app
        .first_run_setup(&t.anonymous(), input("eve@example.com", GOOD_PASSWORD))
        .await;
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[tokio::test]
async fn br_1_1_two_simultaneous_setups_create_exactly_one_admin() {
    let t = TestApp::new();
    let ctx = t.anonymous();
    let (a, b) = tokio::join!(
        t.app
            .first_run_setup(&ctx, input("a@example.com", GOOD_PASSWORD)),
        t.app
            .first_run_setup(&ctx, input("b@example.com", GOOD_PASSWORD)),
    );
    assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
    assert_eq!(t.store.read(|s| s.users.len()), 1);
}

#[tokio::test]
async fn br_2_6_setup_reports_every_problem_without_echoing_the_password() {
    let t = TestApp::new();
    let error = t
        .app
        .first_run_setup(&t.anonymous(), input("not-an-email", "abc"))
        .await
        .unwrap_err();
    let AppError::Validation(issues) = error else {
        panic!("expected validation")
    };
    assert!(issues.iter().any(|i| i.field == "email"));
    assert!(issues.iter().filter(|i| i.field == "password").count() >= 3);
    assert!(issues.iter().all(|i| !i.message.contains("abc")));
    assert_eq!(
        t.store.read(|s| s.users.len()),
        0,
        "nothing is created on bad input"
    );
}

#[tokio::test]
async fn br_2_3_a_breached_password_is_refused() {
    let t = TestApp::new();
    t.breach.add(GOOD_PASSWORD);
    let error = t
        .app
        .first_run_setup(&t.anonymous(), input("ada@example.com", GOOD_PASSWORD))
        .await
        .unwrap_err();
    let AppError::Validation(issues) = error else {
        panic!("expected validation")
    };
    assert!(issues.iter().any(|i| i.message.contains("data breach")));
}

#[tokio::test]
async fn br_9_3_an_unauthenticated_caller_cannot_use_a_signed_in_action() {
    let t = TestApp::new();
    let result = t.app.list_users(&t.anonymous()).await;
    assert!(matches!(result, Err(AppError::Unauthenticated)));
}
