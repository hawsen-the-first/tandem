//! The client address and request identity (SE-12, D16): forwarded headers are believed only when
//! the direct peer is a configured trusted proxy.

use std::net::{IpAddr, SocketAddr};
use tandem_config::TrustedProxies;

/// Where the request really came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientAddr {
    /// The address used for rate limiting and audit entries.
    pub ip: IpAddr,
    /// The direct peer is a trusted proxy, so its forwarded headers were honoured.
    pub via_trusted_proxy: bool,
}

/// The right-most `X-Forwarded-For` entry that is not itself a trusted proxy, when the direct peer
/// is trusted; otherwise the direct peer.
pub fn resolve_client(
    peer: IpAddr,
    forwarded_for: Option<&str>,
    proxies: &TrustedProxies,
) -> ClientAddr {
    if !proxies.contains(peer) {
        return ClientAddr {
            ip: peer,
            via_trusted_proxy: false,
        };
    }
    let from_header = forwarded_for.and_then(|header| {
        header
            .split(',')
            .rev()
            .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
            .find(|candidate| !proxies.contains(*candidate))
    });
    ClientAddr {
        ip: from_header.unwrap_or(peer),
        via_trusted_proxy: true,
    }
}

/// The peer address, or the unspecified address when the connection info is missing (tests).
pub fn peer_ip(connect: Option<SocketAddr>) -> IpAddr {
    connect.map_or(IpAddr::from([0, 0, 0, 0]), |address| address.ip())
}

/// Whether an incoming request ID is acceptable to reuse.
pub fn is_valid_request_id(candidate: &str) -> bool {
    (8..=64).contains(&candidate.len())
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}
