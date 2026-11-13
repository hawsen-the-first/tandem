//! The only HTTP path for calls the server makes on an admin's behalf (SE-18, SE-19, SE-25, RL-7).
//!
//! Every call applies the rules of BR-19: the URL was validated by `OutboundUrl`; names resolve
//! through a resolver that refuses a name if **any** address it returns is link-local or a cloud
//! metadata endpoint; the connection goes to an address that was just checked; redirects only stay
//! on the same scheme, host and port; time, size and TLS are bounded. Nothing here relays remote
//! text: callers map outcomes to fixed statuses (SE-22).

mod client;
mod resolver;

pub use client::{
    Limits, Method, OutboundClient, OutboundError, Request, Response, Target, build_request_url,
};
pub use resolver::{NameLookup, SystemLookup};

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-outbound";

#[cfg(test)]
mod tests;
