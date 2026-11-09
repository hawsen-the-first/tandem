//! The trusted proxy list: addresses and CIDR ranges whose `X-Forwarded-For` is believed.

use std::net::IpAddr;

/// One address or range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    address: IpAddr,
    prefix: u8,
}

/// Why an entry could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an IP address or CIDR range")]
pub struct CidrError(pub String);

impl Cidr {
    /// Parse `10.0.0.0/8`, `::1`, `192.168.1.5` (a single host is a full-length prefix).
    ///
    /// # Errors
    /// [`CidrError`] for anything else, including a prefix longer than the address.
    pub fn parse(text: &str) -> Result<Self, CidrError> {
        let error = || CidrError(text.to_owned());
        let (address_text, prefix_text) = match text.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (text, None),
        };
        let address: IpAddr = address_text.parse().map_err(|_| error())?;
        let longest = if address.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix_text {
            Some(prefix) => prefix.parse::<u8>().map_err(|_| error())?,
            None => longest,
        };
        if prefix > longest {
            return Err(error());
        }
        Ok(Self { address, prefix })
    }

    /// Whether `candidate` lies in this range.
    pub fn contains(&self, candidate: IpAddr) -> bool {
        match (self.address, candidate) {
            (IpAddr::V4(network), IpAddr::V4(candidate)) => same_prefix(
                u128::from(u32::from(network)),
                u128::from(u32::from(candidate)),
                32,
                self.prefix,
            ),
            (IpAddr::V6(network), IpAddr::V6(candidate)) => {
                same_prefix(u128::from(network), u128::from(candidate), 128, self.prefix)
            }
            _ => false,
        }
    }
}

fn same_prefix(left: u128, right: u128, width: u32, prefix: u8) -> bool {
    let prefix = u32::from(prefix);
    if prefix == 0 {
        return true;
    }
    let shift = width - prefix;
    (left >> shift) == (right >> shift)
}

/// The configured list. Empty means no proxy is trusted and forwarded headers are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<Cidr>);

impl TrustedProxies {
    /// Parse a comma-separated list; blanks between commas are ignored.
    ///
    /// # Errors
    /// One [`CidrError`] per bad entry.
    pub fn parse(text: &str) -> Result<Self, Vec<CidrError>> {
        let mut ranges = Vec::new();
        let mut errors = Vec::new();
        for entry in text
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            match Cidr::parse(entry) {
                Ok(range) => ranges.push(range),
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(Self(ranges))
        } else {
            Err(errors)
        }
    }

    /// Whether `address` is a trusted proxy.
    pub fn contains(&self, address: IpAddr) -> bool {
        self.0.iter().any(|range| range.contains(address))
    }

    /// No proxy is trusted.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
