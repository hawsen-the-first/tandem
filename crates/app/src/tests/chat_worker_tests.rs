use super::*;
use crate::JobOutcome;
use tandem_core::chat::Author;
use tandem_core::ids::OutboxEventId;
use tandem_core::job::JobStatus;
use tandem_providers::MockProvider;
use time::Duration;

const MEMBER: &str = "mia@example.com";
const TTL: Duration = Duration::seconds(60);

async fn setup(provider: MockProvider) -> (TestApp, RequestContext, RequestContext) {
    let t = TestApp::with_provider(provider);
    let (admin, _) = full_admin(&t, "ada@example.com").await;
    let (ctx, _) = member(&t, &admin, MEMBER).await;
    (t, ctx, admin)
}

#[tokio::test]
async fn br_13_1_a_skeleton_session_is_visible_only_to_its_owner() {
    let (t, ctx, admin) = setup(MockProvider::new()).await;
    let session = t
        .app
        .create_chat_session(&ctx, "  Planning  ")
        .await
        .unwrap();
    assert_eq!(session.title, "Planning");
    assert_eq!(t.app.list_chat_sessions(&ctx).await.unwrap().len(), 1);
    let (other, _) = member(&t, &admin, "bo@example.com").await;
    assert!(t.app.list_chat_sessions(&other).await.unwrap().is_empty());
    assert!(matches!(
        t.app.post_message(&other, session.id, "hi").await,
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        t.app.list_messages(&other, session.id).await,
        Err(AppError::NotFound)
    ));
}

#[tokio::test]
async fn br_13_1_and_13_2_titles_and_bodies_are_validated() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    assert!(
        matches!(t.app.create_chat_session(&ctx, "  ").await, Err(AppError::Validation(ref i)) if i[0].field == "title")
    );
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    assert!(
        matches!(t.app.post_message(&ctx, session.id, "").await, Err(AppError::Validation(ref i)) if i[0].field == "body")
    );
    assert!(matches!(
        t.app
            .post_message(&ctx, session.id, &"a".repeat(4001))
            .await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn br_13_4_posting_writes_the_message_and_an_outbox_event_together() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    t.app.post_message(&ctx, session.id, "hello").await.unwrap();
    assert_eq!(t.store.read(|s| (s.messages.len(), s.outbox.len())), (1, 1));
}

#[tokio::test]
async fn w15_a_user_message_leads_to_exactly_one_mock_agent_reply_that_does_not_chain() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    let posted = t
        .app
        .post_message(&ctx, session.id, "hello there")
        .await
        .unwrap();
    assert_eq!(t.app.relay_outbox_batch(100).await.unwrap(), 1);
    let outcome = t.app.process_next_job(TTL).await.unwrap();
    assert_eq!(
        outcome,
        JobOutcome::Completed {
            kind: "mock_agent_reply"
        }
    );
    let messages = t.app.list_messages(&ctx, session.id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].body, "Received: hello there");
    assert_eq!(messages[1].author, Author::Agent("mock-agent".to_owned()));
    assert_eq!(messages[1].in_reply_to, Some(posted.id));
    // The reply produced its own event, which must not start another reply.
    assert_eq!(t.app.relay_outbox_batch(100).await.unwrap(), 1);
    assert_eq!(
        t.app.process_next_job(TTL).await.unwrap(),
        JobOutcome::NoJob
    );
    assert_eq!(
        t.app.relay_outbox_batch(100).await.unwrap(),
        0,
        "events are published once"
    );
}

#[tokio::test]
async fn br_13_4_duplicate_delivery_never_produces_a_second_reply() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    t.app.post_message(&ctx, session.id, "hello").await.unwrap();
    t.app.relay_outbox_batch(100).await.unwrap();
    // Simulate at-least-once delivery: the same event is published again.
    t.store.write(|s| s.outbox[0].published_at = None);
    t.app.relay_outbox_batch(100).await.unwrap();
    assert_eq!(t.store.read(|s| s.jobs.len()), 2, "two jobs were queued");
    t.app.process_next_job(TTL).await.unwrap();
    t.app.process_next_job(TTL).await.unwrap();
    let replies = t.store.read(|s| {
        s.messages
            .iter()
            .filter(|m| m.in_reply_to.is_some())
            .count()
    });
    assert_eq!(replies, 1);
}

#[tokio::test]
async fn br_13_3_a_failing_provider_retries_after_5s_30s_2min_then_fails_without_losing_the_message()
 {
    let (t, ctx, _) = setup(MockProvider::failing()).await;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    t.app.post_message(&ctx, session.id, "hello").await.unwrap();
    t.app.relay_outbox_batch(100).await.unwrap();
    for delay in [
        Duration::seconds(5),
        Duration::seconds(30),
        Duration::minutes(2),
    ] {
        let JobOutcome::Retrying { run_at, .. } = t.app.process_next_job(TTL).await.unwrap() else {
            panic!("expected a retry")
        };
        assert_eq!(run_at, t.clock.now_for_assert() + delay);
        assert_eq!(
            t.app.process_next_job(TTL).await.unwrap(),
            JobOutcome::NoJob,
            "not due yet"
        );
        t.clock.advance(delay);
    }
    assert_eq!(
        t.app.process_next_job(TTL).await.unwrap(),
        JobOutcome::Failed {
            kind: "mock_agent_reply"
        }
    );
    assert_eq!(t.store.read(|s| s.jobs[0].status), JobStatus::Failed);
    assert_eq!(
        t.app.list_messages(&ctx, session.id).await.unwrap().len(),
        1,
        "the user's message is kept"
    );
}

#[tokio::test]
async fn br_13_3_a_crashed_workers_lapsed_lease_is_recovered_and_retried() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    t.app.post_message(&ctx, session.id, "hello").await.unwrap();
    t.app.relay_outbox_batch(100).await.unwrap();
    // A worker leased the job and died.
    let now = t.clock.now_for_assert();
    t.store.write(|s| s.jobs[0].lease(now, TTL).unwrap());
    assert_eq!(
        t.app.recover_lapsed_jobs().await.unwrap(),
        0,
        "the lease is still valid"
    );
    t.clock.advance(Duration::seconds(61));
    assert_eq!(t.app.recover_lapsed_jobs().await.unwrap(), 1);
    t.clock.advance(Duration::seconds(5));
    assert_eq!(
        t.app.process_next_job(TTL).await.unwrap(),
        JobOutcome::Completed {
            kind: "mock_agent_reply"
        }
    );
}

#[tokio::test]
async fn br_13_1_the_event_feed_shows_only_the_owners_events_and_flags_old_positions() {
    let (t, ctx, _) = setup(MockProvider::new()).await;
    let owner = ctx.actor().unwrap().user_id;
    let session = t.app.create_chat_session(&ctx, "x").await.unwrap();
    t.app.post_message(&ctx, session.id, "one").await.unwrap();
    t.app.post_message(&ctx, session.id, "two").await.unwrap();
    let page = t
        .app
        .events_for_user(owner, OutboxEventId(0), 10)
        .await
        .unwrap();
    assert_eq!(page.events.len(), 2);
    assert!(!page.resync_required);
    let stranger = user_id(&t, "ada@example.com");
    assert!(
        t.app
            .events_for_user(stranger, OutboxEventId(0), 10)
            .await
            .unwrap()
            .events
            .is_empty()
    );
    assert_eq!(t.app.latest_event_id().await.unwrap(), OutboxEventId(2));
    // Pretend the oldest event was pruned: a client positioned before it must resync.
    t.store.write(|s| {
        s.outbox.remove(0);
    });
    let stale = t
        .app
        .events_for_user(owner, OutboxEventId(0), 10)
        .await
        .unwrap();
    assert!(
        !stale.resync_required,
        "position 0 means a fresh client, not a stale one"
    );
    t.store.write(|s| s.outbox.clear());
    t.app.post_message(&ctx, session.id, "three").await.unwrap();
    t.store.write(|s| s.outbox[0].id = OutboxEventId(10));
    assert!(
        t.app
            .events_for_user(owner, OutboxEventId(3), 10)
            .await
            .unwrap()
            .resync_required
    );
}
