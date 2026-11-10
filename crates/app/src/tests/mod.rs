//! Service tests over the in-memory fakes. Test names carry the business-rule IDs they cover.

mod admin_tests;
mod chat_worker_tests;
mod mfa_tests;
mod password_tests;
mod recovery_tests;
mod session_tests;
mod settings_audit_tests;
mod setup_tests;
mod signin_tests;

use crate::testing::{GOOD_PASSWORD, TestApp};
use crate::{AppError, NewSession, RequestContext, SignInInput, SignInOutcome};
use tandem_core::email::Email;
use tandem_core::ids::UserId;
use tandem_core::role::Role;

/// An admin who has completed setup and MFA, so they hold a full-scope sign-in.
pub(crate) async fn full_admin(t: &TestApp, email: &str) -> (RequestContext, String) {
    let token = t.setup_admin(email).await;
    let enrolment_ctx = t.context_for(&token).await;
    t.enrol_mfa(&enrolment_ctx).await;
    (t.context_for(&token).await, token)
}

/// Invite a member as the admin, accept the invitation, and return their context and token.
pub(crate) async fn member(
    t: &TestApp,
    admin: &RequestContext,
    email: &str,
) -> (RequestContext, String) {
    let invite = t.app.invite(admin, email, Role::Member).await.unwrap();
    let session = t
        .app
        .accept_invitation(&t.anonymous(), &invite.token, "Mia Member", GOOD_PASSWORD)
        .await
        .unwrap();
    (t.context_for(&session.token).await, session.token)
}

pub(crate) fn credentials(email: &str, password: &str) -> SignInInput {
    SignInInput {
        email: email.to_owned(),
        password: password.to_owned(),
        second_factor: None,
    }
}

pub(crate) async fn sign_in_ok(t: &TestApp, email: &str, password: &str) -> NewSession {
    match t
        .app
        .sign_in(&t.anonymous(), credentials(email, password))
        .await
    {
        Ok(SignInOutcome::Success(session)) => *session,
        other => panic!("expected a successful sign-in, got {other:?}"),
    }
}

pub(crate) async fn sign_in_err(t: &TestApp, email: &str, password: &str) -> AppError {
    match t
        .app
        .sign_in(&t.anonymous(), credentials(email, password))
        .await
    {
        Err(error) => error,
        Ok(outcome) => panic!("expected sign-in to fail, got {outcome:?}"),
    }
}

pub(crate) fn user_id(t: &TestApp, email: &str) -> UserId {
    let email = Email::parse(email).unwrap();
    t.store
        .read(|s| s.users.iter().find(|u| u.email == email).map(|u| u.id))
        .unwrap()
}

pub(crate) fn audit_types(t: &TestApp) -> Vec<&'static str> {
    t.store.read(|s| {
        s.audit
            .iter()
            .map(|entry| entry.event.event_type.as_str())
            .collect()
    })
}

pub(crate) fn count_audit(t: &TestApp, event_type: &str) -> usize {
    audit_types(t)
        .into_iter()
        .filter(|ty| *ty == event_type)
        .count()
}

/// `FakeClock::now` through the trait, for assertions.
pub(crate) trait NowForAssert {
    fn now_for_assert(&self) -> time::OffsetDateTime;
}
impl NowForAssert for tandem_support::FakeClock {
    fn now_for_assert(&self) -> time::OffsetDateTime {
        tandem_support::Clock::now(self)
    }
}

#[test]
fn crate_builds() {
    assert!(crate::CRATE_NAME.starts_with("tandem-"));
}
