//! Worker-side services: the outbox relay's decision step, the job runner, and the event feed
//! (W15; BR-13.3, BR-13.4).

use crate::deps::App;
use crate::error::AppError;
use tandem_core::chat::{Author, BODY_MAX_CHARS, ChatMessage};
use tandem_core::ids::{ChatMessageId, JobId, OutboxEventId, UserId};
use tandem_core::job::{FailureOutcome, Job, JobKind};
use tandem_core::outbox::{EventPayload, NewOutboxEvent, OutboxEvent};
use tandem_ports::{ModelRequest, StoreError, Tx};
use time::{Duration, OffsetDateTime};

/// What happened when the runner looked for work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    /// Nothing was due.
    NoJob,
    /// A job finished.
    Completed {
        /// The job kind.
        kind: &'static str,
    },
    /// A job failed and will run again.
    Retrying {
        /// The job kind.
        kind: &'static str,
        /// When it will run again.
        run_at: OffsetDateTime,
    },
    /// A job failed for the last time.
    Failed {
        /// The job kind.
        kind: &'static str,
    },
}

/// Events visible to one user.
#[derive(Debug, Clone)]
pub struct EventPage {
    /// Events after the requested position, in order.
    pub events: Vec<OutboxEvent>,
    /// The client's position is older than the retained history; it must reload its view.
    pub resync_required: bool,
    /// The id of the last event examined, visible or not, so a feed can move past events that
    /// belong to other users instead of rescanning them. Unchanged from `after` when nothing
    /// newer exists.
    pub scanned_up_to: OutboxEventId,
}

impl App {
    /// Publish a batch of unpublished outbox events: run the decision step for each (which may
    /// enqueue jobs) and mark them published, all in one transaction. Returns how many were
    /// processed. Delivery is at-least-once; the handlers are idempotent (BR-13.4).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn relay_outbox_batch(&self, limit: usize) -> Result<usize, AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let events = tx.outbox().fetch_unpublished(limit).await?;
        for event in &events {
            self.decide(tx.as_mut(), event, now).await?;
        }
        let ids: Vec<OutboxEventId> = events.iter().map(|event| event.id).collect();
        if !ids.is_empty() {
            tx.outbox().mark_published(&ids, now).await?;
        }
        tx.commit().await?;
        Ok(events.len())
    }

    /// The decision step: which jobs does this event cause? A user message leads to a mock-agent
    /// reply; an agent message leads to nothing, so replies never chain.
    async fn decide(
        &self,
        tx: &mut dyn Tx,
        event: &OutboxEvent,
        now: OffsetDateTime,
    ) -> Result<(), AppError> {
        match &event.payload {
            EventPayload::MessagePosted {
                message_id,
                author_is_agent: false,
                ..
            } => {
                let job = Job::new(
                    JobId::generate(&*self.clock, &*self.rng)?,
                    JobKind::MockAgentReply {
                        message_id: *message_id,
                    },
                    now,
                    event.trace_context.clone(),
                );
                tx.jobs().enqueue(&job).await?;
            }
            EventPayload::MessagePosted {
                author_is_agent: true,
                ..
            } => {}
        }
        Ok(())
    }

    /// Lease and run the next due job. A failure schedules a retry or ends the job (BR-13.3).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails; job failures are reported in the outcome.
    pub async fn process_next_job(&self, lease_ttl: Duration) -> Result<JobOutcome, AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let Some(mut job) = tx.jobs().lease_next(now, lease_ttl).await? else {
            return Ok(JobOutcome::NoJob);
        };
        tx.commit().await?;

        let kind = job.kind.name();
        let result = self.run_job(&job).await;

        let finished = self.clock.now();
        let outcome = match result {
            Ok(()) => {
                job.complete()
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                JobOutcome::Completed { kind }
            }
            Err(error) => {
                let failure = job
                    .fail(finished, &error.to_string())
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                match failure {
                    FailureOutcome::RetryAt(run_at) => JobOutcome::Retrying { kind, run_at },
                    FailureOutcome::Failed => JobOutcome::Failed { kind },
                }
            }
        };
        let mut tx = self.store.begin().await?;
        tx.jobs().save(&job).await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Re-queue (or fail) jobs whose lease lapsed because a worker died. Returns how many.
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn recover_lapsed_jobs(&self) -> Result<u64, AppError> {
        let mut tx = self.store.begin().await?;
        let recovered = tx.jobs().recover_lapsed_leases(self.clock.now()).await?;
        tx.commit().await?;
        Ok(recovered)
    }

    async fn run_job(&self, job: &Job) -> Result<(), AppError> {
        match &job.kind {
            JobKind::MockAgentReply { message_id } => self.mock_agent_reply(*message_id).await,
        }
    }

    /// Reply to a user message with the scripted provider. Idempotent: a message gets at most one
    /// reply however many times the job runs (BR-13.4).
    async fn mock_agent_reply(&self, message_id: ChatMessageId) -> Result<(), AppError> {
        let message = {
            let mut tx = self.store.begin().await?;
            let Some(message) = tx.chat().get_message(message_id).await? else {
                return Ok(());
            };
            if tx.chat().has_agent_reply_to(message_id).await? {
                return Ok(());
            }
            message
        };
        let response = self
            .provider
            .complete(&ModelRequest {
                prompt: message.body.clone(),
            })
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?;

        let mut tx = self.store.begin().await?;
        if tx.chat().has_agent_reply_to(message_id).await? {
            return Ok(());
        }
        let Some(session) = tx.chat().get_session(message.chat_session_id).await? else {
            return Ok(());
        };
        let now = self.clock.now();
        let reply = ChatMessage {
            id: ChatMessageId::generate(&*self.clock, &*self.rng)?,
            chat_session_id: session.id,
            author: Author::Agent(self.provider.name().to_owned()),
            body: response.text.chars().take(BODY_MAX_CHARS).collect(),
            created_at: now,
            in_reply_to: Some(message_id),
        };
        match tx.chat().insert_message(&reply).await {
            Ok(()) => {}
            Err(StoreError::Conflict(_)) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let event = NewOutboxEvent {
            payload: EventPayload::MessagePosted {
                chat_session_id: session.id,
                message_id: reply.id,
                owner_id: session.owner_id,
                author_is_agent: true,
            },
            trace_context: None,
        };
        tx.outbox().append(&event, now).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Events after `after` that the user may see, for the WebSocket feed and reconnect replay.
    /// Events for other users are filtered out (BR-13.1).
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn events_for_user(
        &self,
        user_id: UserId,
        after: OutboxEventId,
        limit: usize,
    ) -> Result<EventPage, AppError> {
        let mut tx = self.store.begin().await?;
        let lowest = tx.outbox().lowest_retained_id().await?;
        let resync_required = after.0 > 0 && lowest.is_some_and(|lowest| after.0 + 1 < lowest.0);
        let fetched = tx.outbox().fetch_after(after, limit).await?;
        let scanned_up_to = fetched.last().map_or(after, |event| event.id);
        let events = fetched
            .into_iter()
            .filter(|event| event.payload.visible_to() == user_id)
            .collect();
        Ok(EventPage {
            events,
            resync_required,
            scanned_up_to,
        })
    }

    /// The id of the newest event, for a client that connects without a last-seen position.
    ///
    /// # Errors
    /// [`AppError::Internal`] when storage fails.
    pub async fn latest_event_id(&self) -> Result<OutboxEventId, AppError> {
        let mut tx = self.store.begin().await?;
        Ok(tx.outbox().latest_id().await?.unwrap_or(OutboxEventId(0)))
    }
}
