//! HTTP and WebSocket server (api service): the request pipeline, the routes and the OpenAPI
//! description. `main.rs` only wires configuration, services and signals.

pub mod cookie;
pub mod dto;
pub mod error;
pub mod pipeline;
pub mod routes;
pub mod server;
pub mod state;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-api";
