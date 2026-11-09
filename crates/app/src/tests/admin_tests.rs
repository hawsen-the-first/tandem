use super::*;
use tandem_core::invitation::InvitationStatus;
use tandem_core::session::Scope;
use time::Duration;

const MEMBER: &str = "mia@example.com";

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    (t, admin)
}

#[tokio::test]
async fn br_5_1_an_invitation_returns_a_one_time_link_token_and_stores_only_its_hash() {
    let (t, admin) = setup().await;
    let created = t
        .app
        .invite(&admin, "New@Example.com", Role::Member)
        .await
        .unwrap();
    assert_eq!(created.invitation.email.as_str(), "new@example.com");
    let stored = t.store.read(|s| format!("{:?}", s.invitations));
    assert!(
        !stored.contains(&created.token),
        "BR-5.3: the token itself is never stored"
    );
    assert_eq!(count_audit(&t, "user.invited"), 1);
}

#[tokio::test]
async fn br_5_1_duplicate_emails_are_refused() {
    let (t, admin) = setup().await;
    let existing = t.app.invite(&admin, "ada@example.com", Role::Member).await;
    assert!(matches!(existing, Err(AppError::Validation(ref i)) if i[0].field == "email"));
    t.app
        .invite(&admin, "new@example.com", Role::Member)
        .await
        .unwrap();
    let pending = t.app.invite(&admin, "NEW@example.com", Role::Admin).await;
    assert!(matches!(pending, Err(AppError::Validation(_))));
}

#[tokio::test]
async fn br_9_2_members_cannot_invite_and_the_denial_is_audited() {
    let (t, admin) = setup().await;
    let (ctx, _) = member(&t, &admin, MEMBER).await;
    assert!(matches!(
        t.app.invite(&ctx, "x@example.com", Role::Member).await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app.list_users(&ctx).await,
        Err(AppError::NotFound)
    ));
    assert_eq!(count_audit(&t, "authz.denied"), 2);
}

#[tokio::test]
async fn br_5_5_accepting_creates_the_user_with_the_invited_role_and_is_single_use() {
    let (t, admin) = setup().await;
    let created = t.app.invite(&admin, MEMBER, Role::Member).await.unwrap();
    let info = t.app.validate_invitation(&created.token).await.unwrap();
    assert_eq!((info.email.as_str(), info.role), (MEMBER, Role::Member));
    let session = t
        .app
        .accept_invitation(&t.anonymous(), &created.token, "Mia", GOOD_PASSWORD)
        .await
        .unwrap();
    assert_eq!(
        (session.user.role, session.scope),
        (Role::Member, Scope::Full)
    );
    assert!(matches!(
        t.app
            .accept_invitation(&t.anonymous(), &created.token, "Mia", GOOD_PASSWORD)
            .await,
        Err(AppError::InvalidLink)
    ));
    assert_eq!(count_audit(&t, "user.invite_accepted"), 1);
}

#[tokio::test]
async fn br_4_1_an_invited_admin_starts_with_an_enrolment_only_sign_in() {
    let (t, admin) = setup().await;
    let created = t
        .app
        .invite(&admin, "bo@example.com", Role::Admin)
        .await
        .unwrap();
    let session = t
        .app
        .accept_invitation(&t.anonymous(), &created.token, "Bo", GOOD_PASSWORD)
        .await
        .unwrap();
    assert_eq!(
        (session.user.role, session.scope),
        (Role::Admin, Scope::MfaEnrolmentOnly)
    );
}

#[tokio::test]
async fn br_5_2_an_invitation_expires_after_seven_days() {
    let (t, admin) = setup().await;
    let created = t.app.invite(&admin, MEMBER, Role::Member).await.unwrap();
    t.clock.advance(Duration::days(7));
    assert!(matches!(
        t.app.validate_invitation(&created.token).await,
        Err(AppError::InvalidLink)
    ));
    assert!(matches!(
        t.app
            .accept_invitation(&t.anonymous(), &created.token, "Mia", GOOD_PASSWORD)
            .await,
        Err(AppError::InvalidLink)
    ));
    let listed = t.app.list_invitations(&admin).await.unwrap();
    assert_eq!(listed[0].status, InvitationStatus::Expired);
}

#[tokio::test]
async fn br_5_6_a_revoked_invitation_acts_like_an_invalid_link() {
    let (t, admin) = setup().await;
    let created = t.app.invite(&admin, MEMBER, Role::Member).await.unwrap();
    t.app
        .revoke_invitation(&admin, created.invitation.id)
        .await
        .unwrap();
    assert!(matches!(
        t.app.validate_invitation(&created.token).await,
        Err(AppError::InvalidLink)
    ));
    assert!(matches!(
        t.app.revoke_invitation(&admin, created.invitation.id).await,
        Err(AppError::Validation(_))
    ));
    assert_eq!(count_audit(&t, "user.invite_revoked"), 1);
    t.app.invite(&admin, MEMBER, Role::Member).await.unwrap();
}

#[tokio::test]
async fn br_5_4_unknown_tokens_and_weak_passwords_are_handled_without_leaking() {
    let (t, admin) = setup().await;
    assert!(matches!(
        t.app.validate_invitation("nope").await,
        Err(AppError::InvalidLink)
    ));
    assert!(matches!(
        t.app
            .accept_invitation(&t.anonymous(), "nope", "Mia", "weak")
            .await,
        Err(AppError::InvalidLink)
    ));
    let created = t.app.invite(&admin, MEMBER, Role::Member).await.unwrap();
    let weak = t
        .app
        .accept_invitation(&t.anonymous(), &created.token, "Mia", "weak")
        .await;
    assert!(matches!(weak, Err(AppError::Validation(_))));
    assert!(
        t.app.validate_invitation(&created.token).await.is_ok(),
        "the invitation stays usable"
    );
}

#[tokio::test]
async fn br_1_4_and_7_4_deactivation_blocks_sign_in_and_reactivation_restores_it() {
    let (t, admin) = setup().await;
    let (_, token) = member(&t, &admin, MEMBER).await;
    let target = user_id(&t, MEMBER);
    t.app.deactivate_user(&admin, target).await.unwrap();
    assert!(t.app.resolve_session(&token).await.is_err());
    assert_eq!(count_audit(&t, "user.deactivated"), 1);
    assert!(matches!(
        t.app.deactivate_user(&admin, target).await,
        Err(AppError::Validation(_))
    ));
    t.app.reactivate_user(&admin, target).await.unwrap();
    t.clock.advance(Duration::minutes(2));
    sign_in_ok(&t, MEMBER, GOOD_PASSWORD).await;
    assert_eq!(count_audit(&t, "user.reactivated"), 1);
    assert!(matches!(
        t.app.reactivate_user(&admin, target).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn br_7_deactivation_cancels_pending_reset_links() {
    let (t, admin) = setup().await;
    member(&t, &admin, MEMBER).await;
    let target = user_id(&t, MEMBER);
    let link = t.app.issue_reset_link(&admin, target, false).await.unwrap();
    t.app.deactivate_user(&admin, target).await.unwrap();
    assert!(matches!(
        t.app.validate_reset_link(&link.token).await,
        Err(AppError::InvalidLink)
    ));
}

#[tokio::test]
async fn br_7_3_an_admin_cannot_deactivate_themselves() {
    let (t, admin) = setup().await;
    let me = admin.actor().unwrap().user_id;
    let result = t.app.deactivate_user(&admin, me).await;
    assert!(
        matches!(result, Err(AppError::Validation(ref i)) if i[0].message.contains("your own account"))
    );
}

async fn two_admins(t: &TestApp, first: &RequestContext) -> RequestContext {
    let created = t
        .app
        .invite(first, "bo@example.com", Role::Admin)
        .await
        .unwrap();
    let bo = t
        .app
        .accept_invitation(&t.anonymous(), &created.token, "Bo Admin", GOOD_PASSWORD)
        .await
        .unwrap();
    let enrolment_ctx = t.context_for(&bo.token).await;
    t.enrol_mfa(&enrolment_ctx).await;
    t.context_for(&bo.token).await
}

#[tokio::test]
async fn br_7_2_the_last_active_admin_cannot_be_deactivated_or_demoted() {
    let (t, ada) = setup().await;
    let bo = two_admins(&t, &ada).await;
    t.app
        .deactivate_user(&bo, user_id(&t, "ada@example.com"))
        .await
        .unwrap();
    let only_admin = bo.actor().unwrap().user_id;
    let demote = t.app.change_user_role(&bo, only_admin, Role::Member).await;
    assert!(
        matches!(demote, Err(AppError::Validation(ref i)) if i[0].message.contains("At least one active admin"))
    );
    let reactivate_then_try = t
        .app
        .reactivate_user(&bo, user_id(&t, "ada@example.com"))
        .await;
    assert!(reactivate_then_try.is_ok());
}

#[tokio::test]
async fn br_7_2_two_admins_demoting_each_other_at_once_leave_one_admin() {
    let (t, ada) = setup().await;
    let bo = two_admins(&t, &ada).await;
    let (ada_id, bo_id) = (ada.actor().unwrap().user_id, bo.actor().unwrap().user_id);
    let (first, second) = tokio::join!(
        t.app.change_user_role(&ada, bo_id, Role::Member),
        t.app.change_user_role(&bo, ada_id, Role::Member),
    );
    assert_eq!(
        [first.is_ok(), second.is_ok()]
            .iter()
            .filter(|ok| **ok)
            .count(),
        1
    );
    assert_eq!(
        t.store
            .read(|s| s.users.iter().filter(|u| u.is_active_admin()).count()),
        1
    );
}

#[tokio::test]
async fn br_7_1_roles_change_with_old_and_new_values_audited() {
    let (t, admin) = setup().await;
    member(&t, &admin, MEMBER).await;
    let target = user_id(&t, MEMBER);
    t.app
        .change_user_role(&admin, target, Role::Admin)
        .await
        .unwrap();
    let details = t
        .store
        .read(|s| {
            s.audit
                .iter()
                .rev()
                .find(|e| e.event.event_type.as_str() == "user.role_changed")
                .map(|e| e.event.details.clone())
        })
        .unwrap();
    assert_eq!(
        (details["old_role"].as_str(), details["new_role"].as_str()),
        ("member", "admin")
    );
    assert!(matches!(
        t.app.change_user_role(&admin, target, Role::Admin).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn users_are_listed_with_their_mfa_state() {
    let (t, admin) = setup().await;
    member(&t, &admin, MEMBER).await;
    let users = t.app.list_users(&admin).await.unwrap();
    assert_eq!(users.len(), 2);
    assert!(
        users
            .iter()
            .find(|u| u.user.email.as_str() == "ada@example.com")
            .unwrap()
            .mfa_enrolled
    );
    assert!(
        !users
            .iter()
            .find(|u| u.user.email.as_str() == MEMBER)
            .unwrap()
            .mfa_enrolled
    );
}

#[tokio::test]
async fn br_11_1_if_the_audit_entry_cannot_be_written_the_change_does_not_happen() {
    let (t, admin) = setup().await;
    t.store.write(|s| s.fail_audit = true);
    let result = t.app.invite(&admin, "new@example.com", Role::Member).await;
    assert!(matches!(result, Err(AppError::Internal(_))));
    assert_eq!(
        t.store.read(|s| s.invitations.len()),
        0,
        "fail-safe: nothing was stored"
    );
}
