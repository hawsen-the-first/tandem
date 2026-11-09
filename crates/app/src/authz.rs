//! Authorisation (BR-9) and the audit helpers every use case shares.

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::audit::{AuditEntry, AuditEvent, AuditEventType, AuditOutcome};
use tandem_core::ids::AuditEntryId;
use tandem_core::permissions::{Action, Principal, Verdict, evaluate};
use tandem_ports::Tx;

impl App {
    /// Check the permission matrix. Forbidden attempts by signed-in users are audited as
    /// `authz.denied` in their own short transaction.
    pub(crate) async fn authorize(
        &self,
        ctx: &RequestContext,
        action: Action,
    ) -> Result<(), AppError> {
        let first_run_available = if action == Action::FirstRunSetup {
            self.setup_available().await?
        } else {
            false
        };
        match evaluate(ctx.principal(), action, first_run_available) {
            Verdict::Allow => Ok(()),
            verdict => {
                if ctx.principal() == Principal::Anonymous {
                    return Err(match verdict {
                        Verdict::NotFound => AppError::NotFound,
                        _ => AppError::Unauthenticated,
                    });
                }
                self.record_denied(ctx, action).await;
                Err(match verdict {
                    Verdict::NotFound => AppError::NotFound,
                    _ => AppError::Forbidden,
                })
            }
        }
    }

    async fn record_denied(&self, ctx: &RequestContext, action: Action) {
        let event = self
            .audit_event(ctx, AuditEventType::AuthzDenied, AuditOutcome::Denied)
            .detail("action", format!("{action:?}"));
        let result: Result<(), AppError> = async {
            let mut tx = self.store.begin().await?;
            self.append_audit(tx.as_mut(), event).await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            // The request still fails closed; the failed audit write is for operators to see.
            tracing::error!(%error, "could not record an authorisation denial in the audit log");
        }
    }

    /// Start an audit event with the request's actor, ID and source filled in.
    pub(crate) fn audit_event(
        &self,
        ctx: &RequestContext,
        event_type: AuditEventType,
        outcome: AuditOutcome,
    ) -> AuditEvent {
        let event = AuditEvent::new(event_type, outcome, self.clock.now())
            .request(ctx.request_id.clone())
            .source(ctx.source.clone());
        match &ctx.actor {
            Some(actor) => event.actor(actor.user_id, actor.display_name.clone()),
            None => event.actor_label("unknown source"),
        }
    }

    /// Append an audit entry inside the caller's transaction (BR-11.1).
    pub(crate) async fn append_audit(
        &self,
        tx: &mut dyn Tx,
        event: AuditEvent,
    ) -> Result<(), AppError> {
        let id = AuditEntryId::generate(&*self.clock, &*self.rng)?;
        tx.audit().append(&AuditEntry { id, event }).await?;
        Ok(())
    }
}
