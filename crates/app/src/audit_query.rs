//! Reading the audit log (W13, US-2.7). The log is append-only; there is nothing to edit.

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::permissions::Action;
use tandem_ports::{AuditCursor, AuditFilter, AuditPage};

/// Largest page an admin can request.
const MAX_PAGE: usize = 200;

impl App {
    /// A page of audit entries, newest first.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins; [`AppError::Validation`] for an inverted date range.
    pub async fn query_audit(
        &self,
        ctx: &RequestContext,
        filter: AuditFilter,
        after: Option<AuditCursor>,
        limit: usize,
    ) -> Result<AuditPage, AppError> {
        self.authorize(ctx, Action::ViewAudit).await?;
        if let (Some(from), Some(to)) = (filter.from, filter.to)
            && from > to
        {
            return Err(AppError::validation(
                "from",
                "The start date must be before the end date",
            ));
        }
        let mut tx = self.store.begin().await?;
        Ok(tx
            .audit()
            .query(&filter, after, limit.clamp(1, MAX_PAGE))
            .await?)
    }
}
