//! Application services: the use cases of the platform, written against the ports.
//!
//! Every public method on [`App`] corresponds to a workflow in the functional design (W1 to W15).
//! Methods authorise first (BR-9), do their work in one transaction, write the audit entry in that
//! same transaction (BR-11.1), and return typed results or an [`AppError`].

mod admin;
mod audit_query;
mod auth;
mod authz;
mod chat;
mod context;
mod deps;
mod error;
mod mfa;
mod password;
mod recovery;
mod sessions;
mod settings;
mod setup;
mod validation;
mod worker;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use admin::{InvitationInfo, InvitationView, InviteCreated, UserView};
pub use auth::{Me, SecondFactor, SignInInput, SignInOutcome};

pub use context::{Actor, RequestContext};
pub use deps::{App, Dependencies};
pub use error::{AppError, FieldIssue};
pub use mfa::{Enrolment, MfaStatus};
pub use password::{ResetInfo, ResetLink};
pub use sessions::{Authenticated, NewSession};
pub use setup::SetupInput;
pub use worker::{EventPage, JobOutcome};

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-app";

#[cfg(test)]
mod tests;
