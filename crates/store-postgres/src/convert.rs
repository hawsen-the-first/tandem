//! Converting stored text and numbers back into domain values.

use crate::error::corrupt;
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::invitation::InvitationStatus;
use tandem_core::role::{Role, UserStatus};
use tandem_core::session::{EndReason, Scope};
use tandem_core::user::CreatedVia;
use tandem_ports::StoreError;
use time::Duration;

pub(crate) fn email(stored: &str) -> Result<Email, StoreError> {
    Email::parse(stored).map_err(|_| corrupt("email"))
}

pub(crate) fn display_name(stored: &str) -> Result<DisplayName, StoreError> {
    DisplayName::parse(stored).map_err(|_| corrupt("display name"))
}

pub(crate) fn role(stored: &str) -> Result<Role, StoreError> {
    Role::parse(stored).ok_or_else(|| corrupt("role"))
}

pub(crate) fn user_status(stored: &str) -> Result<UserStatus, StoreError> {
    UserStatus::parse(stored).ok_or_else(|| corrupt("user status"))
}

pub(crate) fn created_via(stored: &str) -> Result<CreatedVia, StoreError> {
    CreatedVia::parse(stored).ok_or_else(|| corrupt("creation path"))
}

pub(crate) fn invitation_status(stored: &str) -> Result<InvitationStatus, StoreError> {
    InvitationStatus::parse(stored).ok_or_else(|| corrupt("invitation status"))
}

pub(crate) fn scope(stored: &str) -> Result<Scope, StoreError> {
    Scope::parse(stored).ok_or_else(|| corrupt("scope"))
}

pub(crate) fn end_reason(stored: Option<&str>) -> Result<Option<EndReason>, StoreError> {
    stored
        .map(|text| EndReason::parse(text).ok_or_else(|| corrupt("end reason")))
        .transpose()
}

pub(crate) fn duration(seconds: i64) -> Duration {
    Duration::seconds(seconds)
}

pub(crate) fn step(stored: Option<i64>) -> Result<Option<u64>, StoreError> {
    stored
        .map(|value| u64::try_from(value).map_err(|_| corrupt("time step")))
        .transpose()
}

pub(crate) fn step_to_stored(step: Option<u64>) -> Result<Option<i64>, StoreError> {
    step.map(|value| i64::try_from(value).map_err(|_| corrupt("time step")))
        .transpose()
}
