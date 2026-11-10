use super::*;
use tandem_core::session::EndReason;

#[tokio::test]
async fn br_4_7_the_cli_issues_a_reset_for_the_last_admin_and_audits_it() {
    let t = TestApp::new();
    let (_, token) = full_admin(&t, "ada@example.com").await;
    let link = t
        .app
        .issue_admin_reset_for_cli("Ada@Example.com")
        .await
        .unwrap();
    assert!(t.app.validate_reset_link(&link.token).await.is_ok());
    assert!(t.app.resolve_session(&token).await.is_err(), "sign-ins end");
    let hash = tandem_core::token::hash_token(&token);
    let ended = t.store.read(|s| {
        s.sessions
            .iter()
            .find(|x| x.token_hash == hash)
            .and_then(|x| x.ended_reason)
    });
    assert_eq!(ended, Some(EndReason::ResetIssued));
    assert_eq!(count_audit(&t, "auth.password.reset_issued"), 1);
    let label = t.store.read(|s| {
        s.audit
            .iter()
            .find(|e| e.event.event_type.as_str() == "auth.password.reset_issued")
            .map(|e| e.event.actor_label.clone())
    });
    assert_eq!(label.as_deref(), Some("operations CLI"));
}

#[tokio::test]
async fn br_4_7_the_cli_refuses_when_there_are_several_admins_or_the_user_is_not_an_admin() {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    member(&t, &admin, "mia@example.com").await;
    assert!(matches!(
        t.app.issue_admin_reset_for_cli("mia@example.com").await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app.issue_admin_reset_for_cli("nobody@example.com").await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app.issue_admin_reset_for_cli("not an email").await,
        Err(AppError::Validation(_))
    ));
    let invite = t
        .app
        .invite(&admin, "bea@example.com", tandem_core::role::Role::Admin)
        .await
        .unwrap();
    t.app
        .accept_invitation(&t.anonymous(), &invite.token, "Bea Admin", GOOD_PASSWORD)
        .await
        .unwrap();
    assert!(matches!(
        t.app.issue_admin_reset_for_cli("ada@example.com").await,
        Err(AppError::Validation(_))
    ));
}
