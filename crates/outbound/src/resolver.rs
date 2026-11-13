//! Name resolution that checks every address (BR-19.4, SE-19).

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use tandem_core::outbound_url::is_always_blocked;

/// Looks up a host name. A trait so tests can stand in a resolver that changes its answer
/// (DNS rebinding) or returns a mix of addresses.
pub trait NameLookup: Send + Sync {
    /// Every address the name currently resolves to.
    fn lookup<'a>(
        &'a self,
        host: &'a str,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<IpAddr>>> + Send + 'a>>;
}

/// The operating system's resolver, on the async runtime.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLookup;

impl NameLookup for SystemLookup {
    fn lookup<'a>(
        &'a self,
        host: &'a str,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<IpAddr>>> + Send + 'a>> {
        Box::pin(async move {
            let addresses = tokio::net::lookup_host((host, 0)).await?;
            Ok(addresses.map(|address| address.ip()).collect())
        })
    }
}

/// The error that marks a refused address inside reqwest's error chain.
#[derive(Debug, thiserror::Error)]
#[error("a resolved address is not allowed")]
pub(crate) struct BlockedAddress;

/// Wraps a [`NameLookup`]: refuses the whole name if any address is blocked, returns the rest.
pub(crate) struct CheckedResolver {
    lookup: Arc<dyn NameLookup>,
}

impl CheckedResolver {
    pub(crate) fn new(lookup: Arc<dyn NameLookup>) -> Self {
        Self { lookup }
    }
}

impl Resolve for CheckedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let lookup = Arc::clone(&self.lookup);
        Box::pin(async move {
            let addresses = lookup.lookup(name.as_str()).await?;
            if addresses.is_empty() {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::NotFound,
                    "the name did not resolve",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            if addresses.iter().copied().any(is_always_blocked) {
                return Err(Box::new(BlockedAddress) as Box<dyn std::error::Error + Send + Sync>);
            }
            let sockets: Vec<SocketAddr> = addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, 0))
                .collect();
            Ok(Box::new(sockets.into_iter()) as Addrs)
        })
    }
}
