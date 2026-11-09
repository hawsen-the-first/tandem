//! Instance security settings (W12, US-1.9; BR-10).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::permissions::Action;
use tandem_core::settings::{SecuritySettings, SettingsError};

fn settings_error(error: SettingsError) -> AppError {
    let field = match error {
        SettingsError::IdleOutOfRange | SettingsError::IdleExceedsAbsolute => "idle_timeout",
        SettingsError::AbsoluteOutOfRange => "absolute_duration",
    };
    AppError::validation(field, error.to_string())
}

impl App {
    /// The current security settings.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn get_security_settings(
        &self,
        ctx: &RequestContext,
    ) -> Result<SecuritySettings, AppError> {
        self.authorize(ctx, Action::EditSettings).await?;
        let mut tx = self.store.begin().await?;
        Self::load_settings(tx.as_mut()).await
    }

    /// Replace the security settings. Each changed value is audited with its old and new value.
    /// New sign-ins use the new durations; existing sign-ins keep theirs (BR-8.2).
    ///
    /// # Errors
    /// [`AppError::Validation`] when a value is out of range.
    pub async fn update_security_settings(
        &self,
        ctx: &RequestContext,
        new: SecuritySettings,
    ) -> Result<SecuritySettings, AppError> {
        self.authorize(ctx, Action::EditSettings).await?;
        let actor = ctx.actor()?;
        new.validate().map_err(settings_error)?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let current = Self::load_settings(tx.as_mut()).await?;
        let (old_values, new_values) = (current.to_stored(), new.to_stored());
        for (key, value) in &new_values {
            let old = old_values.get(key).cloned().unwrap_or_default();
            if &old == value {
                continue;
            }
            tx.settings().put(key, value, actor.user_id, now).await?;
            let event = self
                .audit_event(ctx, AuditEventType::SettingsChanged, AuditOutcome::Success)
                .object("setting", key)
                .detail("key", key.as_str())
                .detail("old", old)
                .detail("new", value.as_str());
            self.append_audit(tx.as_mut(), event).await?;
        }
        tx.commit().await?;
        Ok(new)
    }
}
