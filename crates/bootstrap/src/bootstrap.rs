//! Building the application from validated configuration, with the start-up guards of AV-3 and
//! SE-1: wrong schema, insecure database URL or an unusable vault key stops the process.

use std::sync::Arc;
use std::time::Duration;
use tandem_app::{App, Dependencies, Integrations};
use tandem_config::{Config, ConfigError, MasterKeySource};
use tandem_credentials::{
    ArgonHasher, BloomFilter, BreachCheckerImpl, HashProfile, HmacTotp, HttpRangeFetcher,
    OnlineBreachCheck, benchmark_profile, concurrency_limit,
};
use tandem_github::GithubConnector;
use tandem_outbound::OutboundClient;
use tandem_providers::{MockProvider, OpenAiCompatibleProbe};
use tandem_sandbox::DockerRuntimeProbe;
use tandem_search::SearchTester;
use tandem_store_postgres::{
    PgLocks, PgStore, PoolConfig, PoolError, SchemaError, check_schema, connect_pool,
};
use tandem_support::{BreakerConfig, CircuitBreaker, Clock, Rng, SystemClock, SystemRng};
use tandem_telemetry::{InitError, MetricsHandle, MetricsRegistry};
use tandem_vault::EnvelopeVault;

/// Internal setting holding the hex-encoded sealed vault canary.
pub const VAULT_CANARY_KEY: &str = "internal.vault_canary";
/// Internal setting holding the chosen Argon2id profile as `memory_kib:iterations:lanes`.
pub const HASH_PROFILE_KEY: &str = "internal.hash_profile";

const HASH_TARGET: Duration = Duration::from_millis(250);
const HASH_MAX_WAIT: Duration = Duration::from_secs(2);
const WORKER_MAX_POOL: u32 = 5;
const ONLINE_BREACH_TIMEOUT: Duration = Duration::from_secs(2);

/// Which process is starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Process {
    /// The HTTP and WebSocket server.
    Api,
    /// The relay and job runner.
    Worker,
    /// The operator command line.
    Cli,
}

impl Process {
    /// The name used for the database `application_name` and in logs.
    pub fn name(self) -> &'static str {
        match self {
            Self::Api => "tandem-api",
            Self::Worker => "tandem-worker",
            Self::Cli => "tandem-cli",
        }
    }
}

/// Why start-up stopped. Messages carry no secret.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    /// Configuration was invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Logging could not start.
    #[error(transparent)]
    Logging(#[from] InitError),
    /// The database could not be reached securely.
    #[error(transparent)]
    Database(#[from] PoolError),
    /// The schema is not the one this binary expects.
    #[error("{0}")]
    Schema(#[from] SchemaError),
    /// The vault key could not be loaded or does not open the canary.
    #[error("vault: {0}")]
    Vault(String),
    /// Anything else that stops start-up.
    #[error("{0}")]
    Internal(String),
}

/// Start logging and metrics for a process. Call once, before anything logs.
///
/// # Errors
/// [`StartupError`] when logging or metrics were already initialised or the level is bad.
pub fn init_observability(config: &Config) -> Result<MetricsHandle, StartupError> {
    tandem_telemetry::init_logging(&config.log_level)?;
    if config.otlp_endpoint.is_some() {
        tracing::warn!(
            "TANDEM_OTLP_ENDPOINT is set but this build has no OTLP exporter yet; trace IDs are still \
             logged and propagated, nothing is exported"
        );
    }
    MetricsRegistry::new(env!("CARGO_PKG_VERSION"))
        .map_err(|error| StartupError::Internal(error.to_string()))?
        .install()
        .map_err(|_| StartupError::Internal("metrics were already initialised".to_owned()))
}

/// Load the vault from the configured source.
///
/// # Errors
/// [`StartupError::Vault`] with a message that never includes key material.
pub fn load_vault(config: &Config) -> Result<Arc<EnvelopeVault>, StartupError> {
    let rng: Arc<dyn Rng> = Arc::new(SystemRng);
    let vault = match &config.master_key {
        MasterKeySource::File(path) => EnvelopeVault::from_key_file(path, rng),
        MasterKeySource::Inline(secret) => EnvelopeVault::from_key_text(secret.expose(), rng),
    };
    vault
        .map(Arc::new)
        .map_err(|error| StartupError::Vault(error.to_string()))
}

/// Parse `memory_kib:iterations:lanes`; anything malformed or below the minimum is `None`.
pub fn parse_hash_profile(text: &str) -> Option<HashProfile> {
    let mut parts = text.trim().split(':');
    let profile = HashProfile {
        memory_kib: parts.next()?.parse().ok()?,
        iterations: parts.next()?.parse().ok()?,
        lanes: parts.next()?.parse().ok()?,
    };
    let minimum = HashProfile::MINIMUM;
    let acceptable = parts.next().is_none()
        && profile.memory_kib >= minimum.memory_kib
        && profile.iterations >= minimum.iterations
        && profile.lanes >= minimum.lanes
        && profile.argon2().is_ok();
    acceptable.then_some(profile)
}

/// The stored form of a profile.
pub fn format_hash_profile(profile: &HashProfile) -> String {
    format!(
        "{}:{}:{}",
        profile.memory_kib, profile.iterations, profile.lanes
    )
}

/// Everything a running process needs.
pub struct Services {
    /// The application.
    pub app: App,
    /// The store (and through it the pool).
    pub store: PgStore,
    /// The vault.
    pub vault: Arc<EnvelopeVault>,
    /// The validated configuration.
    pub config: Arc<Config>,
    /// The chosen hashing profile.
    pub hash_profile: HashProfile,
    /// The hasher, for its counters.
    pub hasher: Arc<ArgonHasher>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services").finish_non_exhaustive()
    }
}

impl Services {
    /// Connect, run the start-up guards and build the application.
    ///
    /// # Errors
    /// [`StartupError`] on the first guard that fails; the process must not serve traffic.
    pub async fn start(config: Config, process: Process) -> Result<Self, StartupError> {
        let vault = load_vault(&config)?;
        let max_connections = match process {
            Process::Worker => config.database.max_connections.min(WORKER_MAX_POOL),
            Process::Api | Process::Cli => config.database.max_connections,
        };
        let pool = connect_pool(&PoolConfig {
            url: config.database.url.expose().to_owned(),
            ca_file: config.database.ca_file.clone(),
            max_connections,
            acquire_timeout: Duration::from_secs(config.database.acquire_timeout_secs),
            statement_timeout: Duration::from_secs(config.database.statement_timeout_secs),
            application_name: process.name().to_owned(),
        })
        .await?;
        check_schema(&pool).await?;
        let store = PgStore::new(pool);
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        ensure_vault_canary(&store, &vault, &*clock).await?;
        let hash_profile = ensure_hash_profile(&store, &*clock).await?;

        let rng: Arc<dyn Rng> = Arc::new(SystemRng);
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        let concurrency = concurrency_limit(cores, config.hash_memory_budget_mb, &hash_profile);
        let hasher = Arc::new(
            ArgonHasher::new(hash_profile, concurrency, HASH_MAX_WAIT, Arc::clone(&rng))
                .map_err(|error| StartupError::Internal(error.to_string()))?,
        );
        tracing::info!(
            process = process.name(),
            hash_memory_mib = hash_profile.memory_mib(),
            hash_iterations = hash_profile.iterations,
            hash_concurrency = concurrency,
            "password hashing ready"
        );

        let filter = Arc::new(
            BloomFilter::embedded().map_err(|error| StartupError::Internal(error.to_string()))?,
        );
        let breach = match HttpRangeFetcher::new(ONLINE_BREACH_TIMEOUT) {
            Ok(fetcher) => BreachCheckerImpl::with_online(
                filter,
                OnlineBreachCheck::new(
                    Arc::new(fetcher),
                    CircuitBreaker::new(
                        "online_breach_check",
                        BreakerConfig::default(),
                        Arc::clone(&clock),
                    ),
                    ONLINE_BREACH_TIMEOUT,
                ),
            ),
            Err(error) => {
                tracing::warn!(%error, "online breach check unavailable; using the offline filter only");
                BreachCheckerImpl::offline(filter)
            }
        };

        let outbound =
            OutboundClient::new().map_err(|error| StartupError::Internal(error.to_string()))?;
        let integrations = Integrations {
            locks: Arc::new(PgLocks::new(store.pool().clone())),
            provider_probe: Arc::new(OpenAiCompatibleProbe::new(outbound.clone())),
            search_probe: Arc::new(SearchTester::new(outbound.clone())),
            connector_probe: Arc::new(GithubConnector::new(outbound)),
            runtime_probe: Arc::new(DockerRuntimeProbe::new(config.container_socket.clone())),
        };
        let app = App::new(Dependencies {
            store: Arc::new(store.clone()),
            clock,
            rng: Arc::clone(&rng),
            hasher: hasher.clone(),
            breach: Arc::new(breach),
            totp: Arc::new(HmacTotp::new(Arc::clone(&rng))),
            vault: vault.clone(),
            provider: Arc::new(MockProvider::new()),
            issuer: "Tandem".to_owned(),
            integrations,
        });
        Ok(Self {
            app,
            store,
            vault,
            config: Arc::new(config),
            hash_profile,
            hasher,
        })
    }
}

async fn ensure_vault_canary(
    store: &PgStore,
    vault: &EnvelopeVault,
    clock: &dyn Clock,
) -> Result<(), StartupError> {
    let internal = |error: tandem_ports::StoreError| StartupError::Internal(error.to_string());
    if store
        .internal_setting(VAULT_CANARY_KEY)
        .await
        .map_err(internal)?
        .is_none()
    {
        let sealed = vault
            .seal_canary()
            .map_err(|error| StartupError::Vault(error.to_string()))?;
        store
            .set_internal_setting_if_absent(
                VAULT_CANARY_KEY,
                &data_encoding::HEXLOWER.encode(&sealed),
                clock.now(),
            )
            .await
            .map_err(internal)?;
    }
    let stored = store
        .internal_setting(VAULT_CANARY_KEY)
        .await
        .map_err(internal)?
        .ok_or_else(|| StartupError::Vault("the canary could not be stored".to_owned()))?;
    let sealed = data_encoding::HEXLOWER
        .decode(stored.as_bytes())
        .map_err(|_| StartupError::Vault("the stored canary is not valid".to_owned()))?;
    vault.verify_canary(&sealed).map_err(|_| {
        StartupError::Vault(
            "the master key does not open the stored canary: wrong or missing key".to_owned(),
        )
    })
}

async fn ensure_hash_profile(
    store: &PgStore,
    clock: &dyn Clock,
) -> Result<HashProfile, StartupError> {
    let internal = |error: tandem_ports::StoreError| StartupError::Internal(error.to_string());
    if let Some(profile) = store
        .internal_setting(HASH_PROFILE_KEY)
        .await
        .map_err(internal)?
        .as_deref()
        .and_then(parse_hash_profile)
    {
        return Ok(profile);
    }
    let (profile, measured) = tokio::task::spawn_blocking(|| benchmark_profile(HASH_TARGET))
        .await
        .map_err(|error| StartupError::Internal(error.to_string()))?;
    tracing::info!(
        memory_mib = profile.memory_mib(),
        iterations = profile.iterations,
        measured_ms = u64::try_from(measured.as_millis()).unwrap_or(u64::MAX),
        "chose the password hashing profile for this host"
    );
    store
        .set_internal_setting(
            HASH_PROFILE_KEY,
            &format_hash_profile(&profile),
            clock.now(),
        )
        .await
        .map_err(internal)?;
    Ok(profile)
}
