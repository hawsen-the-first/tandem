//! Domain model, state machines and pure business rules. No I/O: the only dependencies are
//! value-level crates and the clock and random-source traits from `tandem-support`.

pub mod admin_rules;
pub mod audit;
pub mod chat;
pub mod config;
pub mod display_name;
pub mod email;
pub mod expiry;
pub mod ids;
pub mod invitation;
pub mod job;
pub mod mfa;
pub mod outbound_url;
pub mod outbox;
pub mod password_policy;
pub mod permissions;
pub mod reset;
pub mod role;
pub mod sandbox_policy;
pub mod session;
pub mod settings;
pub mod throttle;
pub mod token;
pub mod user;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-core";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_builds() {
        assert!(CRATE_NAME.starts_with("tandem-"));
    }
}
