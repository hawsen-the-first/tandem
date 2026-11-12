//! The outbound client (pattern 1 of the U2 NFR design).

use crate::resolver::{BlockedAddress, CheckedResolver, NameLookup, SystemLookup};
use reqwest::StatusCode;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, LOCATION};
use std::error::Error as _;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tandem_core::config::TestStatus;
use tandem_core::outbound_url::OutboundUrl;
use url::Url;

/// Which kind of thing a call is for. Used for metrics and logs only (never a host name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A model provider.
    Provider,
    /// A search provider.
    Search,
    /// GitHub.
    Github,
    /// The container runtime.
    Runtime,
}

impl Target {
    /// The label value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Search => "search",
            Self::Github => "github",
            Self::Runtime => "runtime",
        }
    }
}

/// The request methods the application needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// GET.
    Get,
    /// POST.
    Post,
}

/// Time and size bounds (RL-7, BR-19.5, BR-19.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Time allowed to connect.
    pub connect_timeout: Duration,
    /// Time allowed for the whole call, redirects included.
    pub total_timeout: Duration,
    /// Largest response body read; more is an error.
    pub max_response_bytes: usize,
    /// Redirects followed.
    pub max_redirects: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(15),
            max_response_bytes: 1024 * 1024,
            max_redirects: 3,
        }
    }
}

/// Why a call did not produce a response. None of these carry remote text (SE-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OutboundError {
    /// The address (or a resolved address) is not allowed (BR-19.3).
    #[error("the address is not allowed")]
    RefusedAddress,
    /// Could not connect, resolve the name or complete TLS.
    #[error("the server could not be reached")]
    Unreachable,
    /// Too slow.
    #[error("the server took too long")]
    Timeout,
    /// The response was larger than allowed.
    #[error("the response was too large")]
    TooLarge,
    /// A redirect to somewhere not allowed, or too many redirects.
    #[error("the server redirected somewhere that is not allowed")]
    BadRedirect,
    /// The request itself could not be built (a bad path or header).
    #[error("the request could not be built")]
    InvalidRequest,
}

impl OutboundError {
    /// The fixed test status for this failure (BR-14.6).
    pub const fn test_status(self) -> TestStatus {
        match self {
            Self::RefusedAddress => TestStatus::RefusedAddress,
            Self::Unreachable | Self::InvalidRequest => TestStatus::Unreachable,
            Self::Timeout => TestStatus::Timeout,
            Self::TooLarge | Self::BadRedirect => TestStatus::BadResponse,
        }
    }

    const fn outcome(self) -> &'static str {
        match self {
            Self::RefusedAddress => "refused",
            Self::Unreachable => "unreachable",
            Self::Timeout => "timeout",
            Self::TooLarge => "too_large",
            Self::BadRedirect => "bad_redirect",
            Self::InvalidRequest => "invalid_request",
        }
    }
}

/// A request. Header values may be marked sensitive so they never print.
#[derive(Clone)]
pub struct Request {
    method: Method,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field(
                "headers",
                &self
                    .headers
                    .keys()
                    .map(HeaderName::as_str)
                    .collect::<Vec<_>>(),
            )
            .field("body_bytes", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

impl Request {
    /// A GET with no body.
    pub fn get() -> Self {
        Self {
            method: Method::Get,
            headers: HeaderMap::new(),
            body: None,
        }
    }

    /// A POST with a JSON body.
    pub fn post_json(body: Vec<u8>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Self {
            method: Method::Post,
            headers,
            body: Some(body),
        }
    }

    /// Add a header. Set `sensitive` for anything secret.
    ///
    /// # Errors
    /// [`OutboundError::InvalidRequest`] for a bad name or value.
    pub fn header(
        mut self,
        name: &str,
        value: &str,
        sensitive: bool,
    ) -> Result<Self, OutboundError> {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| OutboundError::InvalidRequest)?;
        let mut value = HeaderValue::from_str(value).map_err(|_| OutboundError::InvalidRequest)?;
        value.set_sensitive(sensitive);
        self.headers.insert(name, value);
        Ok(self)
    }

    /// Add `Authorization: Bearer <token>` as a sensitive header.
    ///
    /// # Errors
    /// [`OutboundError::InvalidRequest`] if the token is not a valid header value.
    pub fn bearer(self, token: &str) -> Result<Self, OutboundError> {
        self.header("authorization", &format!("Bearer {token}"), true)
    }
}

/// A response, fully read within the size cap.
#[derive(Debug, Clone)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    headers: HeaderMap,
    /// The body.
    pub body: Vec<u8>,
}

impl Response {
    /// A response header as text.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Build the URL of a request below a validated base: `path_and_query` must start with `/` and
/// cannot change the scheme, host or port (a defence against `@` and `\` tricks).
///
/// # Errors
/// [`OutboundError::InvalidRequest`] if it could.
pub fn build_request_url(base: &OutboundUrl, path_and_query: &str) -> Result<Url, OutboundError> {
    if !path_and_query.starts_with('/') || path_and_query.contains('#') {
        return Err(OutboundError::InvalidRequest);
    }
    let url = Url::parse(&format!("{}{}", base.as_str(), path_and_query))
        .map_err(|_| OutboundError::InvalidRequest)?;
    let same = url.scheme() == if base.is_https() { "https" } else { "http" }
        && url.host().map(|host| host.to_string()) == Some(base.host_text())
        && url.port_or_known_default() == Some(base.port())
        && url.username().is_empty()
        && url.password().is_none();
    if same {
        Ok(url)
    } else {
        Err(OutboundError::InvalidRequest)
    }
}

/// Why the client could not be built.
#[derive(Debug, thiserror::Error)]
#[error("the outbound HTTP client could not be built: {0}")]
pub struct BuildError(String);

/// The client. Cheap to clone and share.
#[derive(Clone)]
pub struct OutboundClient {
    http: reqwest::Client,
    limits: Limits,
}

impl std::fmt::Debug for OutboundClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboundClient")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

fn classify(error: &reqwest::Error) -> OutboundError {
    if error.is_timeout() {
        return OutboundError::Timeout;
    }
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.is::<BlockedAddress>() {
            return OutboundError::RefusedAddress;
        }
        source = cause.source();
    }
    OutboundError::Unreachable
}

impl OutboundClient {
    /// A client with the standard limits and the system resolver.
    ///
    /// # Errors
    /// [`BuildError`] if the HTTP stack cannot be set up.
    pub fn new() -> Result<Self, BuildError> {
        Self::with(Limits::default(), Arc::new(SystemLookup))
    }

    /// A client with explicit limits and a resolver (tests inject their own).
    ///
    /// # Errors
    /// [`BuildError`] if the HTTP stack cannot be set up.
    #[allow(
        clippy::disallowed_methods,
        reason = "this is the one place an HTTP client is built"
    )]
    pub fn with(limits: Limits, lookup: Arc<dyn NameLookup>) -> Result<Self, BuildError> {
        // reqwest is built without a bundled crypto provider; use `ring`, as the database driver
        // does. Installing twice is harmless.
        let _already_installed = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            .pool_max_idle_per_host(0)
            .user_agent(concat!("Tandem/", env!("CARGO_PKG_VERSION")))
            .dns_resolver(CheckedResolver::new(lookup))
            .build()
            .map_err(|error| BuildError(error.to_string()))?;
        Ok(Self { http, limits })
    }

    /// Make a call below `base`. Redirects stay on the same scheme, host and port, at most
    /// `max_redirects` times; the whole call is bounded by the total timeout.
    ///
    /// # Errors
    /// [`OutboundError`]; an HTTP error status is a successful [`Response`] for the caller to judge.
    pub async fn send(
        &self,
        target: Target,
        base: &OutboundUrl,
        path_and_query: &str,
        request: Request,
    ) -> Result<Response, OutboundError> {
        let started = Instant::now();
        let outcome = match tokio::time::timeout(
            self.limits.total_timeout,
            self.run(base, path_and_query, request),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(OutboundError::Timeout),
        };
        let label = match &outcome {
            Ok(_) => "ok",
            Err(error) => error.outcome(),
        };
        tandem_telemetry::metrics::record_outbound(
            target.as_str(),
            label,
            started.elapsed().as_secs_f64(),
        );
        tracing::debug!(target_kind = target.as_str(), host = %base.host_text(), outcome = label, "outbound call");
        outcome
    }

    async fn run(
        &self,
        base: &OutboundUrl,
        path_and_query: &str,
        request: Request,
    ) -> Result<Response, OutboundError> {
        let mut url = build_request_url(base, path_and_query)?;
        let mut method = request.method;
        let mut headers = request.headers;
        let mut body = request.body;
        let mut redirects = 0_u8;
        loop {
            let mut builder = match method {
                Method::Get => self.http.get(url.clone()),
                Method::Post => self.http.post(url.clone()),
            }
            .headers(headers.clone());
            if let Some(bytes) = &body {
                builder = builder.body(bytes.clone());
            }
            let mut response = builder.send().await.map_err(|error| classify(&error))?;
            let status = response.status();
            if is_redirect(status) {
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
                    .ok_or(OutboundError::BadRedirect)?;
                redirects += 1;
                if redirects > self.limits.max_redirects {
                    return Err(OutboundError::BadRedirect);
                }
                let next = url
                    .join(&location)
                    .map_err(|_| OutboundError::BadRedirect)?;
                let same_place = next.scheme() == url.scheme()
                    && next.host() == url.host()
                    && next.port_or_known_default() == url.port_or_known_default()
                    && next.username().is_empty()
                    && next.password().is_none();
                if !same_place {
                    return Err(OutboundError::BadRedirect);
                }
                if matches!(status.as_u16(), 301..=303) && method == Method::Post {
                    method = Method::Get;
                    body = None;
                    headers.remove(CONTENT_TYPE);
                    headers.remove(CONTENT_LENGTH);
                }
                url = next;
                continue;
            }
            if response.content_length().is_some_and(|length| {
                length > u64::try_from(self.limits.max_response_bytes).unwrap_or(u64::MAX)
            }) {
                return Err(OutboundError::TooLarge);
            }
            let response_headers = response.headers().clone();
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? {
                if bytes.len() + chunk.len() > self.limits.max_response_bytes {
                    return Err(OutboundError::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            return Ok(Response {
                status: status.as_u16(),
                headers: response_headers,
                body: bytes,
            });
        }
    }
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}
