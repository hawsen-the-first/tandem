//! Shared harness: one throw-away PostgreSQL 18 container per test binary, TLS required, the same
//! two roles as the dev environment, migrations applied once to a template database, and a fresh
//! database cloned from it for every test.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, SanType};
use sqlx::AssertSqlSafe;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tandem_store_postgres::{PgStore, PoolConfig, connect_pool, migrate_up};
use tempfile::TempDir;
use tokio::sync::{Mutex, OnceCell};

const IMAGE_TAG: &str = "18.6";
const TEMPLATE: &str = "tandem_template";

/// Plain data about the running cluster (no pools: those belong to each test's runtime).
pub struct Cluster {
    _container_name: String,
    _dir: TempDir,
    pub port: u16,
    pub ca_file: PathBuf,
    pub superuser_password: String,
    pub migrate_password: String,
    pub app_password: String,
}

static CLUSTER: OnceCell<Cluster> = OnceCell::const_new();
static CLONE_LOCK: Mutex<()> = Mutex::const_new(());
static NEXT_DATABASE: AtomicU32 = AtomicU32::new(0);

fn random_password() -> String {
    use std::fmt::Write as _;
    let mut bytes = [0_u8; 18];
    getrandom_fill(&mut bytes);
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn getrandom_fill(bytes: &mut [u8]) {
    use tandem_support::{Rng, SystemRng};
    SystemRng.fill(bytes).expect("system randomness");
}

struct Certificates {
    ca_pem: String,
    server_cert_pem: String,
    server_key_pem: String,
}

fn certificates() -> Certificates {
    let ca_key = KeyPair::generate().expect("CA key");
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "Tandem integration test CA");
    let ca_cert = ca_params.self_signed(&ca_key).expect("CA certificate");
    let issuer = Issuer::new(ca_params, &ca_key);

    let server_key = KeyPair::generate().expect("server key");
    let mut server_params =
        CertificateParams::new(vec!["localhost".to_owned()]).expect("server params");
    server_params
        .subject_alt_names
        .push(SanType::IpAddress(IpAddr::from([127, 0, 0, 1])));
    server_params
        .distinguished_name
        .push(DnType::CommonName, "localhost");
    let server_cert = server_params
        .signed_by(&server_key, &issuer)
        .expect("server certificate");
    Certificates {
        ca_pem: ca_cert.pem(),
        server_cert_pem: server_cert.pem(),
        server_key_pem: server_key.serialize_pem(),
    }
}

const PG_HBA: &str = "\
local   all   all                  trust\n\
hostssl all   all   0.0.0.0/0      scram-sha-256\n\
hostssl all   all   ::/0           scram-sha-256\n";

const ENTRYPOINT_SCRIPT: &str = "\
install -m 600 -o postgres -g postgres /certs/server.key /var/lib/postgresql/server.key && \
exec docker-entrypoint.sh postgres \
-c ssl=on -c ssl_cert_file=/certs/server.crt -c ssl_key_file=/var/lib/postgresql/server.key \
-c ssl_min_protocol_version=TLSv1.2 -c password_encryption=scram-sha-256 \
-c hba_file=/certs/pg_hba.conf";

fn config(port: u16, ca_file: &Path, user: &str, password: &str, database: &str) -> PoolConfig {
    PoolConfig {
        url: format!(
            "postgres://{user}:{password}@127.0.0.1:{port}/{database}?sslmode=verify-full"
        ),
        ca_file: ca_file.to_path_buf(),
        max_connections: 8,
        acquire_timeout: Duration::from_secs(10),
        statement_timeout: Duration::from_secs(30),
        application_name: "tandem-integration-test".to_owned(),
    }
}

async fn start_cluster() -> Cluster {
    let certs = certificates();
    let dir = tempfile::tempdir().expect("temp dir");
    let ca_file = dir.path().join("ca.crt");
    std::fs::write(&ca_file, &certs.ca_pem).expect("write CA");
    let (superuser_password, migrate_password, app_password) =
        (random_password(), random_password(), random_password());

    let certs_dir = dir.path().join("certs");
    std::fs::create_dir(&certs_dir).expect("certs dir");
    for (name, contents) in [
        ("server.crt", certs.server_cert_pem.as_str()),
        ("server.key", certs.server_key_pem.as_str()),
        ("pg_hba.conf", PG_HBA),
    ] {
        std::fs::write(certs_dir.join(name), contents).expect("write certificate material");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&certs_dir, std::fs::Permissions::from_mode(0o755))
            .expect("certs dir permissions");
        for name in ["server.crt", "server.key", "pg_hba.conf"] {
            std::fs::set_permissions(certs_dir.join(name), std::fs::Permissions::from_mode(0o644))
                .expect("certificate permissions");
        }
    }

    let container = format!("tandem-it-{}-{}", std::process::id(), random_password());
    let container = container[..container.len().min(40)].to_owned();
    let run = Command::new("docker")
        .args(["run", "--detach", "--rm", "--name", &container])
        .args(["--publish", "127.0.0.1::5432"])
        .args(["--env", "POSTGRES_PASSWORD"])
        .args(["--volume", &format!("{}:/certs:ro", certs_dir.display())])
        .args(["--entrypoint", "sh"])
        .arg(format!("postgres:{IMAGE_TAG}"))
        .args(["-c", ENTRYPOINT_SCRIPT])
        .env("POSTGRES_PASSWORD", &superuser_password)
        .output()
        .expect("run docker (is it installed?)");
    assert!(
        run.status.success(),
        "docker run failed (is Docker running?): {}",
        String::from_utf8_lossy(&run.stderr)
    );
    // The container is removed when this test process ends, even if it is killed.
    let _ = Command::new("sh")
        .args([
            "-c",
            "while kill -0 \"$1\" 2>/dev/null; do sleep 1; done; docker rm -f \"$2\" >/dev/null 2>&1",
            "watchdog",
            &std::process::id().to_string(),
            &container,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mapping = Command::new("docker")
        .args(["port", &container, "5432/tcp"])
        .output()
        .expect("docker port");
    let mapping = String::from_utf8_lossy(&mapping.stdout);
    let port: u16 = mapping
        .lines()
        .next()
        .and_then(|line| line.rsplit(':').next())
        .and_then(|text| text.trim().parse().ok())
        .expect("mapped port");

    // The log line appears once for the init-time server and once for the real one: retry until the
    // TLS server accepts a verified connection.
    let superuser = config(port, &ca_file, "postgres", &superuser_password, "postgres");
    let mut attempts = 0;
    let admin = loop {
        match connect_pool(&superuser).await {
            Ok(pool) => break pool,
            Err(error) if attempts < 120 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = error;
            }
            Err(error) => panic!("PostgreSQL never became reachable over verified TLS: {error}"),
        }
    };
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {TEMPLATE}")))
        .execute(&admin)
        .await
        .expect("create template");
    admin.close().await;

    let template_admin = connect_pool(&config(
        port,
        &ca_file,
        "postgres",
        &superuser_password,
        TEMPLATE,
    ))
    .await
    .expect("connect to template");
    for statement in [
        format!("CREATE ROLE tandem_migrate LOGIN PASSWORD '{migrate_password}'"),
        format!("CREATE ROLE tandem_app LOGIN PASSWORD '{app_password}'"),
        "CREATE SCHEMA tandem AUTHORIZATION tandem_migrate".to_owned(),
        "GRANT USAGE ON SCHEMA tandem TO tandem_app".to_owned(),
        "ALTER ROLE tandem_migrate SET search_path = tandem".to_owned(),
        "ALTER ROLE tandem_app SET search_path = tandem".to_owned(),
    ] {
        sqlx::query(AssertSqlSafe(statement))
            .execute(&template_admin)
            .await
            .expect("role setup");
    }
    template_admin.close().await;

    let migrator = connect_pool(&config(
        port,
        &ca_file,
        "tandem_migrate",
        &migrate_password,
        TEMPLATE,
    ))
    .await
    .expect("migration role connects");
    migrate_up(&migrator)
        .await
        .expect("migrations apply to the template");
    migrator.close().await;

    Cluster {
        _container_name: container,
        _dir: dir,
        port,
        ca_file,
        superuser_password,
        migrate_password,
        app_password,
    }
}

pub async fn cluster() -> &'static Cluster {
    CLUSTER.get_or_init(start_cluster).await
}

/// A fresh database cloned from the migrated template.
pub struct TestDb {
    pub name: String,
    pub app: sqlx::PgPool,
    pub migrate: sqlx::PgPool,
    pub superuser: sqlx::PgPool,
    pub store: PgStore,
    pub port: u16,
    pub ca_file: PathBuf,
    pub app_password: String,
    pub migrate_password: String,
    pub superuser_password: String,
}

impl TestDb {
    pub async fn new() -> Self {
        Self::build(true).await
    }

    /// An empty database (no tables), for testing migrations themselves.
    pub async fn empty() -> Self {
        Self::build(false).await
    }

    async fn build(from_template: bool) -> Self {
        let cluster = cluster().await;
        let name = format!(
            "t{}_{}",
            std::process::id(),
            NEXT_DATABASE.fetch_add(1, Ordering::SeqCst)
        );
        let admin = connect_pool(&config(
            cluster.port,
            &cluster.ca_file,
            "postgres",
            &cluster.superuser_password,
            "postgres",
        ))
        .await
        .expect("admin pool");
        {
            let _guard = CLONE_LOCK.lock().await;
            let statement = if from_template {
                format!("CREATE DATABASE {name} TEMPLATE {TEMPLATE}")
            } else {
                format!("CREATE DATABASE {name}")
            };
            sqlx::query(AssertSqlSafe(statement))
                .execute(&admin)
                .await
                .expect("create database");
        }
        admin.close().await;
        let owner = |user: &str, password: &str| {
            config(cluster.port, &cluster.ca_file, user, password, &name)
        };
        let app = connect_pool(&owner("tandem_app", &cluster.app_password))
            .await
            .expect("app pool");
        let migrate = connect_pool(&owner("tandem_migrate", &cluster.migrate_password))
            .await
            .expect("migrate pool");
        let superuser = connect_pool(&owner("postgres", &cluster.superuser_password))
            .await
            .expect("superuser pool");
        if !from_template {
            // An empty database needs the schema the init script gives the real one.
            sqlx::query("CREATE SCHEMA IF NOT EXISTS tandem AUTHORIZATION tandem_migrate")
                .execute(&superuser)
                .await
                .expect("schema");
            sqlx::query("GRANT USAGE ON SCHEMA tandem TO tandem_app")
                .execute(&superuser)
                .await
                .expect("usage");
        }
        Self {
            store: PgStore::new(app.clone()),
            name,
            app,
            migrate,
            superuser,
            port: cluster.port,
            ca_file: cluster.ca_file.clone(),
            app_password: cluster.app_password.clone(),
            migrate_password: cluster.migrate_password.clone(),
            superuser_password: cluster.superuser_password.clone(),
        }
    }

    pub fn app_config(&self) -> PoolConfig {
        config(
            self.port,
            &self.ca_file,
            "tandem_app",
            &self.app_password,
            &self.name,
        )
    }
}

impl TestDb {
    /// A `verify-full` URL for one of the roles on this database.
    pub fn url(&self, user: &str, password: &str) -> String {
        format!(
            "postgres://{user}:{password}@127.0.0.1:{}/{}?sslmode=verify-full",
            self.port, self.name
        )
    }

    pub fn app_url(&self) -> String {
        self.url("tandem_app", &self.app_password)
    }

    pub fn migrate_url(&self) -> String {
        self.url("tandem_migrate", &self.migrate_password)
    }

    /// A 32-byte test key as a one-line key file.
    pub const KEY_FILE_TEXT: &'static str =
        "k1 current AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\n";

    /// Configuration environment for the app role, with files written into `dir`.
    pub fn environment(&self, dir: &Path) -> std::collections::BTreeMap<String, String> {
        let key_file = dir.join("master.key");
        std::fs::write(&key_file, Self::KEY_FILE_TEXT).expect("write key file");
        [
            (
                "TANDEM_PUBLIC_URL",
                "https://tandem.example.test".to_owned(),
            ),
            ("TANDEM_DATABASE_URL", self.app_url()),
            (
                "TANDEM_DATABASE_CA_FILE",
                self.ca_file.display().to_string(),
            ),
            ("TANDEM_MASTER_KEY_FILE", key_file.display().to_string()),
            ("TANDEM_WORKER_LISTEN_ADDR", "127.0.0.1:0".to_owned()),
            ("TANDEM_LOG_LEVEL", "warn".to_owned()),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
    }
}
