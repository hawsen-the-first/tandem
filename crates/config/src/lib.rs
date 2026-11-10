//! Configuration loader: environment, optional TOML file and secret files.
//!
//! Precedence is environment, then the TOML file named by `TANDEM_CONFIG`, then defaults.
//! Secrets are accepted only from the environment or `*_FILE` variables; one found in the TOML
//! file is an error. Every problem is reported, one message each, and no message carries a secret.

mod proxies;
mod secret;

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use url::Url;

pub use proxies::{Cidr, CidrError, TrustedProxies};
pub use secret::Secret;

/// Name of this crate, used by smoke tests and diagnostics.
pub const CRATE_NAME: &str = "tandem-config";

const PREFIX: &str = "TANDEM_";

/// Settings that may appear in the TOML file (key without the prefix, lower case).
const FILE_KEYS: &[&str] = &[
    "public_url",
    "database_ca_file",
    "master_key_file",
    "listen_addr",
    "worker_listen_addr",
    "trusted_proxies",
    "otlp_endpoint",
    "log_level",
    "hash_memory_budget_mb",
    "db_max_connections",
    "db_acquire_timeout_secs",
    "db_statement_timeout_secs",
];

/// Settings that are secrets: never accepted from the TOML file.
const SECRET_KEYS: &[&str] = &["database_url", "master_key", "metrics_token"];

const LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];
const MIN_METRICS_TOKEN_LEN: usize = 16;

/// One problem found while loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem {
    /// The setting (`TANDEM_...` name or TOML key).
    pub setting: String,
    /// What is wrong, without any secret value.
    pub message: String,
}

/// Every problem found; start-up refuses to continue when this is returned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ConfigError {
    /// The problems, in a stable order.
    pub problems: Vec<ConfigProblem>,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid configuration ({} problem", self.problems.len())?;
        if self.problems.len() != 1 {
            f.write_str("s")?;
        }
        f.write_str(")")?;
        for problem in &self.problems {
            write!(f, "\n  {}: {}", problem.setting, problem.message)?;
        }
        Ok(())
    }
}

/// Where the vault key material comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterKeySource {
    /// A key file path.
    File(PathBuf),
    /// The key file text supplied directly (from the environment or a `*_FILE` secret).
    Inline(Secret),
}

/// The database connection settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseSettings {
    /// The connection URL (carries the password).
    pub url: Secret,
    /// The CA certificate used to verify the server.
    pub ca_file: PathBuf,
    /// Maximum pooled connections for this process.
    pub max_connections: u32,
    /// How long to wait for a connection.
    pub acquire_timeout_secs: u64,
    /// Per-statement timeout.
    pub statement_timeout_secs: u64,
}

/// The validated configuration, built once at start-up and passed down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The public origin of the site (scheme, host, port; no path).
    pub public_url: Url,
    /// Database settings.
    pub database: DatabaseSettings,
    /// Vault key source.
    pub master_key: MasterKeySource,
    /// The address the API listens on.
    pub listen_addr: SocketAddr,
    /// The address the worker serves its health and metrics endpoints on.
    pub worker_listen_addr: SocketAddr,
    /// Proxies whose forwarded headers are believed.
    pub trusted_proxies: TrustedProxies,
    /// Bearer token for `/metrics`; the endpoint is disabled without one.
    pub metrics_token: Option<Secret>,
    /// OTLP endpoint; export is off without one.
    pub otlp_endpoint: Option<Url>,
    /// Log level.
    pub log_level: String,
    /// Memory the password-hashing gate may use, in MiB.
    pub hash_memory_budget_mb: u32,
}

impl Config {
    /// Load from the process environment (and the TOML file it names).
    ///
    /// # Errors
    /// [`ConfigError`] listing every problem.
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = std::env::vars()
            .filter(|(name, _)| name.starts_with(PREFIX))
            .collect();
        Self::load(&environment)
    }

    /// Load from an explicit environment map (names include the `TANDEM_` prefix).
    ///
    /// # Errors
    /// [`ConfigError`] listing every problem.
    pub fn load(environment: &BTreeMap<String, String>) -> Result<Self, ConfigError> {
        let mut loader = Loader::new(environment);
        loader.merge_file();
        let config = loader.build();
        match config {
            Some(config) if loader.problems.is_empty() => Ok(config),
            _ => Err(ConfigError {
                problems: loader.problems,
            }),
        }
    }

    /// The public origin as a WebSocket origin (`ws`/`wss`), for the content security policy.
    pub fn public_websocket_origin(&self) -> String {
        let scheme = if self.public_url.scheme() == "https" {
            "wss"
        } else {
            "ws"
        };
        let host = self.public_url.host_str().unwrap_or_default();
        match self.public_url.port() {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        }
    }

    /// The public origin as the `Origin` header carries it (no trailing slash).
    pub fn public_origin(&self) -> String {
        self.public_url.origin().ascii_serialization()
    }
}

/// Read one secret from `NAME` or `NAME_FILE` in `environment` without loading the whole
/// configuration (for the operations CLI, which needs only a database credential).
///
/// # Errors
/// [`ConfigError`] when both forms are set, or the file is missing or empty.
pub fn secret_from_env(
    environment: &BTreeMap<String, String>,
    name: &str,
) -> Result<Option<Secret>, ConfigError> {
    let mut loader = Loader::new(environment);
    let secret = loader.secret(name);
    if loader.problems.is_empty() {
        Ok(secret)
    } else {
        Err(ConfigError {
            problems: loader.problems,
        })
    }
}

struct Loader {
    /// Environment values, `TANDEM_` names.
    environment: BTreeMap<String, String>,
    /// Values from the TOML file under their `TANDEM_` names (lower precedence).
    file: BTreeMap<String, String>,
    problems: Vec<ConfigProblem>,
}

impl Loader {
    fn new(environment: &BTreeMap<String, String>) -> Self {
        Self {
            environment: environment.clone(),
            file: BTreeMap::new(),
            problems: Vec::new(),
        }
    }

    fn problem(&mut self, setting: &str, message: impl Into<String>) {
        self.problems.push(ConfigProblem {
            setting: setting.to_owned(),
            message: message.into(),
        });
    }

    fn merge_file(&mut self) {
        let Some(path) = self.environment.get("TANDEM_CONFIG").cloned() else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                self.problem(
                    "TANDEM_CONFIG",
                    format!("cannot read {path}: {}", error.kind()),
                );
                return;
            }
        };
        let table: toml::Table = match text.parse() {
            Ok(table) => table,
            Err(error) => {
                // The parser message quotes source lines, which could hold a secret: report only
                // where the problem is.
                let place = error
                    .span()
                    .map(|span| text[..span.start.min(text.len())].lines().count().max(1))
                    .map_or_else(String::new, |line| format!(" near line {line}"));
                self.problem("TANDEM_CONFIG", format!("{path} is not valid TOML{place}"));
                return;
            }
        };
        for (key, value) in &table {
            let lower = key.to_ascii_lowercase();
            if SECRET_KEYS.contains(&lower.as_str()) {
                self.problem(
                    key,
                    "secrets must come from the environment or a *_FILE variable, not the \
                     configuration file",
                );
            } else if FILE_KEYS.contains(&lower.as_str()) {
                let text = match value {
                    toml::Value::String(text) => Some(text.clone()),
                    toml::Value::Integer(number) => Some(number.to_string()),
                    toml::Value::Array(items) => items
                        .iter()
                        .map(|item| item.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                        .map(|items| items.join(",")),
                    _ => None,
                };
                match text {
                    Some(text) => {
                        self.file
                            .insert(format!("{PREFIX}{}", lower.to_ascii_uppercase()), text);
                    }
                    None => self.problem(key, "has a value of the wrong type"),
                }
            } else {
                self.problem(key, "is not a known setting");
            }
        }
    }

    /// The effective plain value: environment first, then the file.
    fn value(&self, name: &str) -> Option<String> {
        self.environment
            .get(name)
            .or_else(|| self.file.get(name))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    /// A secret from `NAME` or `NAME_FILE` (environment only).
    fn secret(&mut self, name: &str) -> Option<Secret> {
        let file_name = format!("{name}_FILE");
        let direct = self
            .environment
            .get(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let from_file = self
            .environment
            .get(&file_name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        match (direct, from_file) {
            (Some(_), Some(_)) => {
                self.problem(name, format!("set both {name} and {file_name}; use one"));
                None
            }
            (Some(value), None) => Some(Secret::new(value)),
            (None, Some(path)) => match std::fs::read_to_string(&path) {
                Ok(text) if text.trim().is_empty() => {
                    self.problem(&file_name, format!("{path} is empty"));
                    None
                }
                Ok(text) => Some(Secret::new(text.trim_end_matches(['\n', '\r']))),
                Err(error) => {
                    self.problem(&file_name, format!("cannot read {path}: {}", error.kind()));
                    None
                }
            },
            (None, None) => None,
        }
    }

    fn number<T>(&mut self, name: &str, default: T, low: T, high: T) -> T
    where
        T: std::str::FromStr + PartialOrd + Copy + std::fmt::Display,
    {
        let Some(text) = self.value(name) else {
            return default;
        };
        match text.parse::<T>() {
            Ok(number) if number >= low && number <= high => number,
            Ok(_) => {
                self.problem(name, format!("must be between {low} and {high}"));
                default
            }
            Err(_) => {
                self.problem(name, "must be a whole number");
                default
            }
        }
    }

    fn path_that_exists(&mut self, name: &str) -> Option<PathBuf> {
        let Some(text) = self.value(name) else {
            self.problem(name, "is required");
            return None;
        };
        let path = PathBuf::from(&text);
        if path.is_file() {
            Some(path)
        } else {
            self.problem(name, format!("{text} is not a readable file"));
            None
        }
    }

    fn public_url(&mut self) -> Option<Url> {
        let name = "TANDEM_PUBLIC_URL";
        let Some(text) = self.value(name) else {
            self.problem(name, "is required");
            return None;
        };
        let url = match Url::parse(&text) {
            Ok(url) => url,
            Err(_) => {
                self.problem(name, "is not a valid URL");
                return None;
            }
        };
        if !matches!(url.scheme(), "http" | "https") {
            self.problem(name, "must start with https:// (or http:// for loopback)");
            return None;
        }
        let Some(host) = url.host() else {
            self.problem(name, "must include a host");
            return None;
        };
        if url.scheme() == "http" && !is_loopback(&host) {
            self.problem(
                name,
                "must use https:// unless the host is localhost or a loopback address",
            );
            return None;
        }
        if (url.path() != "/" && !url.path().is_empty())
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            self.problem(
                name,
                "must be an origin only: no path, query, fragment or credentials",
            );
            return None;
        }
        Some(url)
    }

    fn database_url(&mut self) -> Option<Secret> {
        let name = "TANDEM_DATABASE_URL";
        let Some(secret) = self.secret(name) else {
            if !self
                .problems
                .iter()
                .any(|problem| problem.setting.starts_with(name))
            {
                self.problem(name, "is required (or TANDEM_DATABASE_URL_FILE)");
            }
            return None;
        };
        let Ok(url) = Url::parse(secret.expose()) else {
            self.problem(name, "is not a valid URL");
            return None;
        };
        if !matches!(url.scheme(), "postgres" | "postgresql") {
            self.problem(name, "must be a postgres:// URL");
            return None;
        }
        let mode = url
            .query_pairs()
            .find(|(key, _)| key == "sslmode")
            .map(|(_, value)| value.into_owned());
        if !matches!(mode.as_deref(), Some("verify-full" | "verify-ca")) {
            self.problem(
                name,
                "must require a verified TLS connection: add ?sslmode=verify-full",
            );
            return None;
        }
        Some(secret)
    }

    fn master_key(&mut self) -> Option<MasterKeySource> {
        let inline = self
            .environment
            .get("TANDEM_MASTER_KEY")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(Secret::new);
        let file = self.value("TANDEM_MASTER_KEY_FILE");
        match (inline, file) {
            (Some(_), Some(_)) => {
                self.problem(
                    "TANDEM_MASTER_KEY_FILE",
                    "set both TANDEM_MASTER_KEY and TANDEM_MASTER_KEY_FILE; use one",
                );
                None
            }
            (Some(secret), None) => Some(MasterKeySource::Inline(secret)),
            (None, Some(_)) => self
                .path_that_exists("TANDEM_MASTER_KEY_FILE")
                .map(MasterKeySource::File),
            (None, None) => {
                if !self
                    .problems
                    .iter()
                    .any(|problem| problem.setting.starts_with("TANDEM_MASTER_KEY"))
                {
                    self.problem("TANDEM_MASTER_KEY_FILE", "is required");
                }
                None
            }
        }
    }

    fn build(&mut self) -> Option<Config> {
        let public_url = self.public_url();
        let database_url = self.database_url();
        let ca_file = self.path_that_exists("TANDEM_DATABASE_CA_FILE");
        let master_key = self.master_key();

        let listen_addr = match self.value("TANDEM_LISTEN_ADDR") {
            None => SocketAddr::from(([127, 0, 0, 1], 8080)),
            Some(text) => text.parse().unwrap_or_else(|_| {
                self.problem("TANDEM_LISTEN_ADDR", "must look like 127.0.0.1:8080");
                SocketAddr::from(([127, 0, 0, 1], 8080))
            }),
        };

        let worker_listen_addr = match self.value("TANDEM_WORKER_LISTEN_ADDR") {
            None => SocketAddr::from(([127, 0, 0, 1], 8081)),
            Some(text) => text.parse().unwrap_or_else(|_| {
                self.problem("TANDEM_WORKER_LISTEN_ADDR", "must look like 127.0.0.1:8081");
                SocketAddr::from(([127, 0, 0, 1], 8081))
            }),
        };

        let trusted_proxies = match self.value("TANDEM_TRUSTED_PROXIES") {
            None => TrustedProxies::default(),
            Some(text) => TrustedProxies::parse(&text).unwrap_or_else(|errors| {
                for error in errors {
                    self.problem("TANDEM_TRUSTED_PROXIES", error.to_string());
                }
                TrustedProxies::default()
            }),
        };

        let metrics_token = self.secret("TANDEM_METRICS_TOKEN");
        if let Some(token) = &metrics_token
            && token.expose().len() < MIN_METRICS_TOKEN_LEN
        {
            self.problem(
                "TANDEM_METRICS_TOKEN",
                format!("must be at least {MIN_METRICS_TOKEN_LEN} characters"),
            );
        }

        let otlp_endpoint = match self.value("TANDEM_OTLP_ENDPOINT") {
            None => None,
            Some(text) => match Url::parse(&text) {
                Ok(url) if matches!(url.scheme(), "http" | "https") && url.host().is_some() => {
                    Some(url)
                }
                _ => {
                    self.problem("TANDEM_OTLP_ENDPOINT", "must be an http:// or https:// URL");
                    None
                }
            },
        };

        let log_level = self
            .value("TANDEM_LOG_LEVEL")
            .map_or_else(|| "info".to_owned(), |level| level.to_ascii_lowercase());
        let log_level = if LOG_LEVELS.contains(&log_level.as_str()) {
            log_level
        } else {
            self.problem(
                "TANDEM_LOG_LEVEL",
                format!("must be one of {}", LOG_LEVELS.join(", ")),
            );
            "info".to_owned()
        };

        let hash_memory_budget_mb =
            self.number("TANDEM_HASH_MEMORY_BUDGET_MB", 512_u32, 64, 65_536);
        let max_connections = self.number("TANDEM_DB_MAX_CONNECTIONS", 10_u32, 1, 100);
        let acquire_timeout_secs = self.number("TANDEM_DB_ACQUIRE_TIMEOUT_SECS", 5_u64, 1, 60);
        let statement_timeout_secs =
            self.number("TANDEM_DB_STATEMENT_TIMEOUT_SECS", 30_u64, 1, 300);

        Some(Config {
            public_url: public_url?,
            database: DatabaseSettings {
                url: database_url?,
                ca_file: ca_file?,
                max_connections,
                acquire_timeout_secs,
                statement_timeout_secs,
            },
            master_key: master_key?,
            listen_addr,
            worker_listen_addr,
            trusted_proxies,
            metrics_token,
            otlp_endpoint,
            log_level,
            hash_memory_budget_mb,
        })
    }
}

fn is_loopback(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(address) => IpAddr::V4(*address).is_loopback(),
        url::Host::Ipv6(address) => IpAddr::V6(*address).is_loopback(),
    }
}

#[cfg(test)]
mod tests;
