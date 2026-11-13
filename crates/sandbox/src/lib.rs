//! Sandbox support. U2 contains only the container runtime probe (LC-29): one `GET /version` over
//! the configured Unix socket, so an admin can see whether the container tier is available before
//! choosing it. The runners arrive in a later unit.
//!
//! Access to a container runtime socket is root-equivalent on the host (SE-23), so the probe asks
//! one fixed question, reads at most 64 KiB, gives up after 3 seconds, and keeps only a sanitised
//! name and version from the answer.

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tandem_core::config::{ProbeDetails, ProbeResult, ProbeSubject, TestStatus};
use tandem_ports::RuntimeProbe;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_ANSWER_BYTES: usize = 64 * 1024;
const REQUEST: &[u8] = b"GET /version HTTP/1.0\r\nHost: container-runtime\r\nAccept: application/json\r\nConnection: close\r\n\r\n";

/// Probes a Docker-compatible runtime at a Unix socket.
#[derive(Debug, Clone)]
pub struct DockerRuntimeProbe {
    socket: Option<PathBuf>,
    timeout: Duration,
}

impl DockerRuntimeProbe {
    /// A probe for the socket in the configuration, or none when it is not configured.
    pub fn new(socket: Option<PathBuf>) -> Self {
        Self {
            socket,
            timeout: TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl RuntimeProbe for DockerRuntimeProbe {
    async fn probe(&self) -> ProbeResult {
        let Some(socket) = &self.socket else {
            return ProbeResult::new(ProbeSubject::Runtime, TestStatus::NotConfigured);
        };
        let outcome = tokio::time::timeout(self.timeout, ask(socket)).await;
        match outcome {
            Err(_) => ProbeResult::new(ProbeSubject::Runtime, TestStatus::Timeout),
            Ok(Err(status)) => ProbeResult::new(ProbeSubject::Runtime, status),
            Ok(Ok(details)) => {
                let mut result = ProbeResult::new(ProbeSubject::Runtime, TestStatus::Ok);
                result.details = details;
                result
            }
        }
    }
}

#[cfg(unix)]
async fn ask(socket: &std::path::Path) -> Result<ProbeDetails, TestStatus> {
    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|_| TestStatus::Unreachable)?;
    stream
        .write_all(REQUEST)
        .await
        .map_err(|_| TestStatus::Unreachable)?;
    let mut answer = Vec::new();
    let read = (&mut stream)
        .take(u64::try_from(MAX_ANSWER_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut answer)
        .await;
    if read.is_err() {
        return Err(TestStatus::Unreachable);
    }
    if answer.len() > MAX_ANSWER_BYTES {
        return Err(TestStatus::BadResponse);
    }
    parse(&answer)
}

#[cfg(not(unix))]
async fn ask(_socket: &std::path::Path) -> Result<ProbeDetails, TestStatus> {
    Err(TestStatus::Unreachable)
}

fn parse(answer: &[u8]) -> Result<ProbeDetails, TestStatus> {
    let split = answer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(TestStatus::BadResponse)?;
    let head = std::str::from_utf8(&answer[..split]).map_err(|_| TestStatus::BadResponse)?;
    let status_line = head.lines().next().ok_or(TestStatus::BadResponse)?;
    let mut parts = status_line.split_whitespace();
    let http = parts.next().unwrap_or_default();
    if !http.starts_with("HTTP/1.") || parts.next() != Some("200") {
        return Err(TestStatus::BadResponse);
    }
    let body: Value =
        serde_json::from_slice(&answer[split + 4..]).map_err(|_| TestStatus::BadResponse)?;
    let version = body
        .get("Version")
        .and_then(Value::as_str)
        .ok_or(TestStatus::BadResponse)?;
    let name = body
        .pointer("/Platform/Name")
        .and_then(Value::as_str)
        .or_else(|| body.pointer("/Components/0/Name").and_then(Value::as_str));
    Ok(ProbeDetails {
        runtime_name: name.and_then(clean_name),
        runtime_version: clean_version(version),
        ..ProbeDetails::default()
    })
}

/// A short display name made only of letters, digits, spaces and a few separators.
fn clean_name(text: &str) -> Option<String> {
    let text = text.trim();
    let allowed = |character: char| {
        character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '-' | '_')
    };
    (!text.is_empty() && text.len() <= 64 && text.chars().all(allowed)).then(|| text.to_owned())
}

/// A version such as `27.0.3` or `5.2.1-rc1+build`.
fn clean_version(text: &str) -> Option<String> {
    let text = text.trim();
    let allowed = |character: char| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+')
    };
    (!text.is_empty() && text.len() <= 40 && text.chars().all(allowed)).then(|| text.to_owned())
}

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-sandbox";

#[cfg(test)]
#[cfg(unix)]
mod tests;
