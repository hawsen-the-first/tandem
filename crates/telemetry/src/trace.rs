//! W3C trace context: the trace ID that appears in every log line and travels in outbox rows and
//! jobs so a trace continues from a request into the worker (OB-2).

use tandem_support::{Rng, RngError, random_array};

/// A trace position: trace ID, span ID, sampled flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceContext {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    sampled: bool,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0_u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn non_zero<const N: usize>(rng: &dyn Rng) -> Result<[u8; N], RngError> {
    loop {
        let candidate = random_array::<N>(rng)?;
        if candidate.iter().any(|byte| *byte != 0) {
            return Ok(candidate);
        }
    }
}

impl TraceContext {
    /// Start a new trace.
    ///
    /// # Errors
    /// [`RngError`] when the system random source fails.
    pub fn new_root(rng: &dyn Rng) -> Result<Self, RngError> {
        Ok(Self {
            trace_id: non_zero(rng)?,
            span_id: non_zero(rng)?,
            sampled: true,
        })
    }

    /// The same trace with a fresh span ID.
    ///
    /// # Errors
    /// [`RngError`] when the system random source fails.
    pub fn child(&self, rng: &dyn Rng) -> Result<Self, RngError> {
        Ok(Self {
            span_id: non_zero(rng)?,
            ..*self
        })
    }

    /// Parse a `traceparent` header value. Anything malformed is `None` (never trust, start fresh).
    pub fn parse(header: &str) -> Option<Self> {
        let mut parts = header.trim().split('-');
        let (version, trace, span, flags) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || version != "00" {
            return None;
        }
        let trace_id = unhex::<16>(trace)?;
        let span_id = unhex::<8>(span)?;
        let flags = unhex::<1>(flags)?;
        if trace_id.iter().all(|byte| *byte == 0) || span_id.iter().all(|byte| *byte == 0) {
            return None;
        }
        Some(Self {
            trace_id,
            span_id,
            sampled: flags[0] & 1 == 1,
        })
    }

    /// The `traceparent` form.
    pub fn to_header(&self) -> String {
        format!(
            "00-{}-{}-{}",
            hex(&self.trace_id),
            hex(&self.span_id),
            if self.sampled { "01" } else { "00" }
        )
    }

    /// The trace ID as 32 lower-case hex characters.
    pub fn trace_id(&self) -> String {
        hex(&self.trace_id)
    }

    /// The span ID as 16 lower-case hex characters.
    pub fn span_id(&self) -> String {
        hex(&self.span_id)
    }
}
