//! The rules for URLs the server calls on an admin's behalf (BR-19, SE-19): provider base URLs, the
//! SearXNG address and similar. Pure: no name resolution happens here. The outbound client applies
//! the same address rules again to every address a name resolves to.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::{Host, Url};

/// Longest URL accepted.
pub const MAX_URL_LENGTH: usize = 2048;

/// Where an address lives, for the purposes of BR-19.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressClass {
    /// 127.0.0.0/8, ::1 and the unspecified address (which reaches this host).
    Loopback,
    /// RFC 1918, carrier-grade NAT (100.64.0.0/10, used by VPN overlays) and IPv6 unique-local.
    Private,
    /// Anything routable on the public internet.
    Public,
    /// Link-local: 169.254.0.0/16 and fe80::/10. Never allowed.
    LinkLocal,
    /// A cloud metadata endpoint that is not link-local. Never allowed.
    Metadata,
    /// Multicast, broadcast and other addresses that are not a single host. Never allowed.
    Unusable,
}

/// Addresses of cloud metadata services that are not covered by the link-local ranges.
const METADATA_V4: [Ipv4Addr; 4] = [
    Ipv4Addr::new(100, 100, 100, 200), // Alibaba Cloud
    Ipv4Addr::new(168, 63, 129, 16),   // Azure wire server
    Ipv4Addr::new(192, 0, 0, 192),     // Oracle Cloud (legacy)
    Ipv4Addr::new(169, 254, 169, 254), // AWS, GCP, Azure, others (also link-local)
];
const METADATA_V6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254);

fn classify_v4(address: Ipv4Addr) -> AddressClass {
    let octets = address.octets();
    if METADATA_V4.contains(&address) {
        AddressClass::Metadata
    } else if address.is_loopback() || address.is_unspecified() {
        AddressClass::Loopback
    } else if address.is_link_local() {
        AddressClass::LinkLocal
    } else if address.is_multicast() || address.is_broadcast() || octets[0] >= 240 {
        AddressClass::Unusable
    } else if address.is_private() || (octets[0] == 100 && (octets[1] & 0xc0) == 64) {
        AddressClass::Private
    } else {
        AddressClass::Public
    }
}

/// The IPv4 address hidden inside an IPv6 address (mapped, compatible or NAT64), if any.
fn embedded_v4(address: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = address.segments();
    // ::ffff:a.b.c.d (mapped) and ::a.b.c.d (compatible), excluding :: and ::1.
    if let Some(mapped) = address.to_ipv4_mapped() {
        return Some(mapped);
    }
    if segments[..6] == [0, 0, 0, 0, 0, 0] && segments[6..] != [0, 0] && segments[6..] != [0, 1] {
        return address.to_ipv4();
    }
    // 64:ff9b::/96 (NAT64) embeds an IPv4 address in the last 32 bits.
    if segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
        let [high, low] = [segments[6], segments[7]];
        let bytes = [high.to_be_bytes(), low.to_be_bytes()];
        return Some(Ipv4Addr::new(
            bytes[0][0],
            bytes[0][1],
            bytes[1][0],
            bytes[1][1],
        ));
    }
    None
}

/// Classify an address. Addresses that embed an IPv4 address are classified as that address.
pub fn classify(address: IpAddr) -> AddressClass {
    match address {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = embedded_v4(v6) {
                return classify_v4(v4);
            }
            let first = v6.segments()[0];
            if v6 == METADATA_V6 {
                AddressClass::Metadata
            } else if v6.is_loopback() || v6.is_unspecified() {
                AddressClass::Loopback
            } else if (first & 0xffc0) == 0xfe80 {
                AddressClass::LinkLocal
            } else if v6.is_multicast() {
                AddressClass::Unusable
            } else if (first & 0xfe00) == 0xfc00 {
                AddressClass::Private
            } else {
                AddressClass::Public
            }
        }
    }
}

/// Whether an address may never be connected to, whatever the URL says (BR-19.3).
pub fn is_always_blocked(address: IpAddr) -> bool {
    matches!(
        classify(address),
        AddressClass::LinkLocal | AddressClass::Metadata | AddressClass::Unusable
    )
}

/// Why a URL was refused. The text is shown to the admin next to the field (BR-19.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UrlRefusal {
    /// Nothing was entered.
    #[error("Enter a URL")]
    Empty,
    /// Longer than [`MAX_URL_LENGTH`].
    #[error("That URL is too long")]
    TooLong,
    /// Not a URL.
    #[error("That is not a valid URL")]
    Malformed,
    /// Neither `http` nor `https`.
    #[error("Use an http:// or https:// URL")]
    UnsupportedScheme,
    /// No host.
    #[error("The URL needs a host name")]
    NoHost,
    /// A user name or password in the URL.
    #[error("Do not put a user name or password in the URL")]
    Credentials,
    /// A query string or fragment.
    #[error("Remove everything after ? or # from the URL")]
    QueryOrFragment,
    /// `http://` to a public host.
    #[error(
        "Use https:// for public servers; http:// is only for local and private-network servers"
    )]
    HttpToPublicHost,
    /// A link-local or metadata address.
    #[error(
        "That address is not allowed (link-local and cloud-metadata addresses are always blocked)"
    )]
    BlockedAddress,
    /// Multicast, broadcast and similar.
    #[error("That address cannot be used for a server")]
    UnusableAddress,
}

/// A validated, normalised URL (BR-19.1, BR-19.2, BR-19.3). The only way to call out for an admin.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OutboundUrl {
    text: String,
    scheme_is_https: bool,
    host: Host<String>,
    port: u16,
}

impl fmt::Debug for OutboundUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OutboundUrl").field(&self.text).finish()
    }
}

impl fmt::Display for OutboundUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Names that are local by convention: `localhost`, `*.localhost`, single-label names, `*.local`
/// and `*.internal`.
fn is_local_name(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name == "localhost"
        || name.ends_with(".localhost")
        || !name.contains('.')
        || name.ends_with(".local")
        || name.ends_with(".internal")
}

impl OutboundUrl {
    /// Parse and normalise an admin-entered URL.
    ///
    /// # Errors
    /// [`UrlRefusal`] with the reason.
    pub fn parse(input: &str) -> Result<Self, UrlRefusal> {
        let input = input.trim();
        if input.is_empty() {
            return Err(UrlRefusal::Empty);
        }
        if input.len() > MAX_URL_LENGTH {
            return Err(UrlRefusal::TooLong);
        }
        let url = Url::parse(input).map_err(|_| UrlRefusal::Malformed)?;
        let https = match url.scheme() {
            "https" => true,
            "http" => false,
            _ => return Err(UrlRefusal::UnsupportedScheme),
        };
        let host = url.host().ok_or(UrlRefusal::NoHost)?.to_owned();
        if !url.username().is_empty() || url.password().is_some() {
            return Err(UrlRefusal::Credentials);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(UrlRefusal::QueryOrFragment);
        }
        let literal = match &host {
            Host::Ipv4(address) => Some(IpAddr::V4(*address)),
            Host::Ipv6(address) => Some(IpAddr::V6(*address)),
            Host::Domain(_) => None,
        };
        if let Some(address) = literal {
            match classify(address) {
                AddressClass::LinkLocal | AddressClass::Metadata => {
                    return Err(UrlRefusal::BlockedAddress);
                }
                AddressClass::Unusable => return Err(UrlRefusal::UnusableAddress),
                AddressClass::Public if !https => return Err(UrlRefusal::HttpToPublicHost),
                _ => {}
            }
        } else if let Host::Domain(name) = &host
            && !https
            && !is_local_name(name)
        {
            return Err(UrlRefusal::HttpToPublicHost);
        }
        let port = url.port_or_known_default().ok_or(UrlRefusal::Malformed)?;
        let mut text = url.as_str().to_owned();
        while text.ends_with('/') {
            text.pop();
        }
        Ok(Self {
            text,
            scheme_is_https: https,
            host,
            port,
        })
    }

    /// The normalised URL: without trailing slashes.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Whether the scheme is `https`.
    pub fn is_https(&self) -> bool {
        self.scheme_is_https
    }

    /// The host.
    pub fn host(&self) -> &Host<String> {
        &self.host
    }

    /// The port, explicit or the scheme's default.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The address, when the host is an address literal rather than a name.
    pub fn literal_address(&self) -> Option<IpAddr> {
        match &self.host {
            Host::Ipv4(address) => Some(IpAddr::V4(*address)),
            Host::Ipv6(address) => Some(IpAddr::V6(*address)),
            Host::Domain(_) => None,
        }
    }

    /// The host name or address as text, for logs (never the path, which may carry nothing secret
    /// but is not needed).
    pub fn host_text(&self) -> String {
        self.host.to_string()
    }

    /// A URL for `path` below this one (`path` starts with `/`).
    pub fn join(&self, path: &str) -> String {
        format!("{}{}", self.text, path)
    }
}

#[cfg(test)]
mod tests;
