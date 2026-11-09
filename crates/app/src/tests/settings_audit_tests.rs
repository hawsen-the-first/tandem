use super::*;
use tandem_core::audit::AuditEventType;
use tandem_core::settings::SecuritySettings;
use tandem_ports::AuditFilter;
use time::Duration;

async fn setup() -> (TestApp, RequestContext) {
    let t = TestApp::new();
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    (t, admin)
}

#[tokio::test]
async fn br_10_2_defaults_apply_until_something_is_stored() {
    let (t, admin) = setup().await;
    assert_eq!(
        t.app.get_security_settings(&admin).await.unwrap(),
        SecuritySettings::default()
    );
}

#[tokio::test]
async fn br_10_3_each_changed_value_is_audited_with_old_and_new() {
    let (t, admin) = setup().await;
    let changed = SecuritySettings {
        idle_timeout: Duration::hours(2),
        online_breach_check: true,
        ..SecuritySettings::default()
    };
    t.app
        .update_security_settings(&admin, changed)
        .await
        .unwrap();
    assert_eq!(t.app.get_security_settings(&admin).await.unwrap(), changed);
    assert_eq!(count_audit(&t, "settings.changed"), 2);
    let entry = t
        .store
        .read(|s| {
            s.audit
                .iter()
                .find(|e| {
                    e.event.details.get("key").map(String::as_str) == Some("signin.idle_timeout")
                })
                .cloned()
        })
        .unwrap();
    assert_eq!(entry.event.details["old"], "43200");
    assert_eq!(entry.event.details["new"], "7200");
}

#[tokio::test]
async fn br_10_1_out_of_range_values_are_refused_and_an_unchanged_save_audits_nothing() {
    let (t, admin) = setup().await;
    let too_short = SecuritySettings {
        idle_timeout: Duration::minutes(5),
        ..SecuritySettings::default()
    };
    assert!(
        matches!(t.app.update_security_settings(&admin, too_short).await, Err(AppError::Validation(ref i)) if i[0].field == "idle_timeout")
    );
    let too_long = SecuritySettings {
        absolute_duration: Duration::days(120),
        ..SecuritySettings::default()
    };
    assert!(
        matches!(t.app.update_security_settings(&admin, too_long).await, Err(AppError::Validation(ref i)) if i[0].field == "absolute_duration")
    );
    t.app
        .update_security_settings(&admin, SecuritySettings::default())
        .await
        .unwrap();
    assert_eq!(count_audit(&t, "settings.changed"), 0);
}

#[tokio::test]
async fn br_9_2_members_cannot_see_or_change_settings_or_the_audit_log() {
    let (t, admin) = setup().await;
    let (ctx, _) = member(&t, &admin, "mia@example.com").await;
    assert!(matches!(
        t.app.get_security_settings(&ctx).await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app
            .update_security_settings(&ctx, SecuritySettings::default())
            .await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app
            .query_audit(&ctx, AuditFilter::default(), None, 10)
            .await,
        Err(AppError::NotFound)
    ));
}

#[tokio::test]
async fn br_11_audit_pages_are_newest_first_and_chain_with_a_cursor() {
    let (t, admin) = setup().await;
    for i in 0..5 {
        t.clock.advance(Duration::seconds(1));
        t.app
            .invite(&admin, &format!("user{i}@example.com"), Role::Member)
            .await
            .unwrap();
    }
    let first = t
        .app
        .query_audit(&admin, AuditFilter::default(), None, 3)
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 3);
    assert!(first.entries[0].event.occurred_at >= first.entries[1].event.occurred_at);
    let next = t
        .app
        .query_audit(&admin, AuditFilter::default(), first.next, 200)
        .await
        .unwrap();
    assert!(next.next.is_none());
    let total = t.store.read(|s| s.audit.len());
    assert_eq!(first.entries.len() + next.entries.len(), total);
}

#[tokio::test]
async fn br_11_audit_filters_by_event_type_actor_and_dates() {
    let (t, admin) = setup().await;
    t.clock.advance(Duration::hours(1));
    let created = t
        .app
        .invite(&admin, "x@example.com", Role::Member)
        .await
        .unwrap();
    t.app
        .revoke_invitation(&admin, created.invitation.id)
        .await
        .unwrap();
    let only_revokes = AuditFilter {
        event_type: Some(AuditEventType::UserInviteRevoked),
        ..AuditFilter::default()
    };
    assert_eq!(
        t.app
            .query_audit(&admin, only_revokes, None, 50)
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    let by_actor = AuditFilter {
        actor_id: Some(admin.actor().unwrap().user_id),
        ..AuditFilter::default()
    };
    assert!(
        t.app
            .query_audit(&admin, by_actor, None, 50)
            .await
            .unwrap()
            .entries
            .len()
            >= 2
    );
    let recent = AuditFilter {
        from: Some(t.clock.now_for_assert() - Duration::minutes(1)),
        ..AuditFilter::default()
    };
    assert_eq!(
        t.app
            .query_audit(&admin, recent, None, 50)
            .await
            .unwrap()
            .entries
            .len(),
        2
    );
    let inverted = AuditFilter {
        from: Some(t.clock.now_for_assert()),
        to: Some(t.clock.now_for_assert() - Duration::days(1)),
        ..AuditFilter::default()
    };
    assert!(matches!(
        t.app.query_audit(&admin, inverted, None, 50).await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn br_11_4_no_secret_ever_appears_in_the_audit_log() {
    let (t, admin) = setup().await;
    let created = t
        .app
        .invite(&admin, "x@example.com", Role::Member)
        .await
        .unwrap();
    t.app
        .issue_reset_link(&admin, admin.actor().unwrap().user_id, false)
        .await
        .unwrap();
    sign_in_err(&t, "ada@example.com", "Wrong-Password-1").await;
    let stored = t.store.read(|s| format!("{:?}", s.audit));
    assert!(!stored.contains(&created.token));
    assert!(!stored.contains(GOOD_PASSWORD));
    assert!(!stored.contains("Wrong-Password-1"));
}
