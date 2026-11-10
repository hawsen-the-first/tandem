//! The CLI against a real PostgreSQL. Needs Docker.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use clap::Parser;
use common::TestDb;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tandem_bootstrap::{Process, Services};
use tandem_cli::{Cli, CliError, Output, run};
use tandem_core::display_name::DisplayName;
use tandem_core::email::Email;
use tandem_core::ids::UserId;
use tandem_core::role::{Role, UserStatus};
use tandem_core::user::{CreatedVia, User};
use tandem_ports::{Store, Vault};
use tandem_support::{Clock, FakeClock, FakeRng, Rng, SystemRng};
use tandem_vault::EnvelopeVault;
use time::{Duration, OffsetDateTime};

const KEY_ONE: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";
const KEY_TWO: &str = "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=";

#[derive(Default)]
struct Capture(Vec<String>);

impl Output for Capture {
    fn line(&mut self, text: &str) {
        self.0.push(text.to_owned());
    }
}

async fn invoke(
    arguments: &[&str],
    environment: &BTreeMap<String, String>,
) -> (Result<(), CliError>, Vec<String>) {
    let cli = Cli::try_parse_from(std::iter::once("tandem").chain(arguments.iter().copied()))
        .expect("arguments parse");
    let mut output = Capture::default();
    let result = run(cli, environment, &mut output).await;
    (result, output.0)
}

fn minimal_environment(url: String, ca: &Path, url_name: &str) -> BTreeMap<String, String> {
    [
        (url_name.to_owned(), url),
        (
            "TANDEM_DATABASE_CA_FILE".to_owned(),
            ca.display().to_string(),
        ),
    ]
    .into_iter()
    .collect()
}

async fn insert_user(db: &TestDb, email: &str, role: Role, seed: u64) -> User {
    let clock = FakeClock::at_unix(1_800_000_000);
    let rng = FakeRng::new(seed);
    let user = User {
        id: UserId::generate(&clock, &rng).unwrap(),
        email: Email::parse(email).unwrap(),
        display_name: DisplayName::parse("Test Person").unwrap(),
        role,
        status: UserStatus::Active,
        password_hash: "$argon2id$stub".to_owned(),
        password_changed_at: clock.now(),
        created_at: clock.now(),
        created_via: CreatedVia::Invitation,
        deactivated_at: None,
        deactivated_by: None,
        last_sign_in_at: None,
    };
    let mut tx = db.store.begin().await.unwrap();
    tx.users().insert(&user).await.unwrap();
    tx.commit().await.unwrap();
    user
}

async fn preset_hash_profile(db: &TestDb) {
    sqlx::query(
        "INSERT INTO tandem.instance_settings (key, value, updated_at)
         VALUES ('internal.hash_profile', '19456:2:1', now()) ON CONFLICT DO NOTHING",
    )
    .execute(&db.superuser)
    .await
    .unwrap();
}

async fn audit_count(db: &TestDb) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM tandem.audit_entries")
        .fetch_one(&db.superuser)
        .await
        .unwrap()
}

#[tokio::test]
async fn migrate_up_and_down_apply_and_revert_the_schema() {
    let db = TestDb::empty().await;
    let environment = minimal_environment(db.migrate_url(), &db.ca_file, "TANDEM_DATABASE_URL");

    let (result, output) = invoke(&["migrate", "up"], &environment).await;
    result.unwrap();
    assert!(output[0].contains("schema is at version 4"), "{output:?}");
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'tandem'",
    )
    .fetch_one(&db.superuser)
    .await
    .unwrap();
    assert!(tables > 10);

    let (refused, _) = invoke(&["migrate", "down"], &environment).await;
    assert!(matches!(refused, Err(CliError::NotConfirmed(_))));
    let (result, output) = invoke(&["migrate", "down", "--yes"], &environment).await;
    result.unwrap();
    assert!(output[0].contains("reverted migration 4"), "{output:?}");

    let (result, output) = invoke(&["migrate", "up"], &environment).await;
    result.unwrap();
    assert!(output[0].contains("version 4"), "{output:?}");

    let (missing, _) = invoke(&["migrate", "up"], &BTreeMap::new()).await;
    assert!(matches!(
        missing,
        Err(CliError::Missing("TANDEM_DATABASE_URL"))
    ));
}

#[tokio::test]
async fn vault_rekey_moves_every_sealed_value_to_the_current_key_atomically() {
    let db = TestDb::new().await;
    let dir = tempfile::tempdir().unwrap();
    let rng: Arc<dyn Rng> = Arc::new(SystemRng);
    let old =
        EnvelopeVault::from_key_text(&format!("k1 current {KEY_ONE}\n"), Arc::clone(&rng)).unwrap();
    let new_only =
        EnvelopeVault::from_key_text(&format!("k2 current {KEY_TWO}\n"), Arc::clone(&rng)).unwrap();

    let user = insert_user(&db, "ada@example.test", Role::Member, 1).await;
    let secret = b"twenty-byte-totp-key".to_vec();
    let sealed = old
        .seal("mfa_secret", &user.id.to_string(), &secret)
        .unwrap();
    sqlx::query("INSERT INTO tandem.mfa_credentials (user_id, secret_sealed, confirmed_at) VALUES ($1::uuid, $2, now())")
        .bind(user.id.to_string())
        .bind(sealed)
        .execute(&db.superuser)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tandem.instance_settings (key, value, updated_at) VALUES ('internal.vault_canary', $1, now())")
        .bind(data_encoding_hex(&old.seal_canary().unwrap()))
        .execute(&db.superuser)
        .await
        .unwrap();

    let mut environment = db.environment(dir.path());
    // A key file that lacks the old key cannot open the stored values: nothing changes.
    let only_new = dir.path().join("only-new.key");
    std::fs::write(&only_new, format!("k2 current {KEY_TWO}\n")).unwrap();
    environment.insert(
        "TANDEM_MASTER_KEY_FILE".to_owned(),
        only_new.display().to_string(),
    );
    let (refused, _) = invoke(&["vault", "rekey"], &environment).await;
    let message = refused.unwrap_err().to_string();
    assert!(message.contains("decrypt-only"), "{message}");
    assert!(!message.contains(KEY_ONE) && !message.contains(KEY_TWO));
    let unchanged: Vec<u8> = sqlx::query_scalar("SELECT secret_sealed FROM tandem.mfa_credentials")
        .fetch_one(&db.superuser)
        .await
        .unwrap();
    assert_eq!(
        old.open("mfa_secret", &user.id.to_string(), &unchanged)
            .unwrap(),
        secret
    );

    let both = dir.path().join("both.key");
    std::fs::write(
        &both,
        format!("k2 current {KEY_TWO}\nk1 decrypt-only {KEY_ONE}\n"),
    )
    .unwrap();
    environment.insert(
        "TANDEM_MASTER_KEY_FILE".to_owned(),
        both.display().to_string(),
    );
    let (result, output) = invoke(&["vault", "rekey"], &environment).await;
    result.unwrap();
    assert!(
        output[0].contains("re-sealed 1 MFA secret(s) and the vault canary"),
        "{output:?}"
    );

    let resealed: Vec<u8> = sqlx::query_scalar("SELECT secret_sealed FROM tandem.mfa_credentials")
        .fetch_one(&db.superuser)
        .await
        .unwrap();
    assert_eq!(
        new_only
            .open("mfa_secret", &user.id.to_string(), &resealed)
            .unwrap(),
        secret
    );
    let canary: String = sqlx::query_scalar(
        "SELECT value FROM tandem.instance_settings WHERE key = 'internal.vault_canary'",
    )
    .fetch_one(&db.superuser)
    .await
    .unwrap();
    new_only.verify_canary(&hex_decode(&canary)).unwrap();

    let (result, output) = invoke(&["vault", "rekey"], &environment).await;
    result.unwrap();
    assert!(
        output[0].contains("re-sealed 0 MFA secret(s)"),
        "{output:?}"
    );
}

fn data_encoding_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap())
        .collect()
}

#[tokio::test]
async fn issue_reset_serves_only_the_last_admin_and_prints_a_working_link() {
    let db = TestDb::new().await;
    preset_hash_profile(&db).await;
    let dir = tempfile::tempdir().unwrap();
    let environment = db.environment(dir.path());
    insert_user(&db, "ada@example.test", Role::Admin, 10).await;
    insert_user(&db, "mia@example.test", Role::Member, 11).await;

    let (unknown, _) = invoke(
        &["admin", "issue-reset", "--email", "nobody@example.test"],
        &environment,
    )
    .await;
    assert!(unknown.unwrap_err().to_string().contains("no active admin"));
    let (member, _) = invoke(
        &["admin", "issue-reset", "--email", "mia@example.test"],
        &environment,
    )
    .await;
    assert!(member.unwrap_err().to_string().contains("no active admin"));

    let (result, output) = invoke(
        &["admin", "issue-reset", "--email", "ada@example.test"],
        &environment,
    )
    .await;
    result.unwrap();
    let link = output.last().unwrap();
    let token = link
        .strip_prefix("https://tandem.example.test/reset-password?token=")
        .unwrap_or_else(|| panic!("unexpected link {link}"));
    let config = tandem_config::Config::load(&environment).unwrap();
    let services = Services::start(config, Process::Cli).await.unwrap();
    services.app.validate_reset_link(token).await.unwrap();
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM tandem.audit_entries WHERE event_type = 'auth.password.reset_issued' AND actor_label = 'operations CLI'")
        .fetch_one(&db.superuser)
        .await
        .unwrap();
    assert_eq!(audited, 1);

    insert_user(&db, "bea@example.test", Role::Admin, 12).await;
    let (several, _) = invoke(
        &["admin", "issue-reset", "--email", "ada@example.test"],
        &environment,
    )
    .await;
    assert!(
        several
            .unwrap_err()
            .to_string()
            .contains("More than one admin")
    );
}

#[tokio::test]
async fn audit_prune_needs_a_privileged_role_a_safe_cutoff_and_confirmation() {
    let db = TestDb::new().await;
    for age_days in [200_i32, 10] {
        sqlx::query(
            "INSERT INTO tandem.audit_entries (id, occurred_at, actor_label, event_type, outcome)
             VALUES (gen_random_uuid(), now() - make_interval(days => $1), 'test', 'auth.setup', 'success')",
        )
        .bind(age_days)
        .execute(&db.superuser)
        .await
        .unwrap();
    }
    let privileged = minimal_environment(
        db.migrate_url(),
        &db.ca_file,
        "TANDEM_PRIVILEGED_DATABASE_URL",
    );
    let cutoff = (OffsetDateTime::now_utc() - Duration::days(100))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let recent = (OffsetDateTime::now_utc() - Duration::days(30))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();

    let (too_recent, _) = invoke(
        &["audit", "prune", "--before", &recent, "--yes"],
        &privileged,
    )
    .await;
    assert!(matches!(too_recent, Err(CliError::BadArgument(_))));
    let (unconfirmed, _) = invoke(&["audit", "prune", "--before", &cutoff], &privileged).await;
    assert!(matches!(unconfirmed, Err(CliError::NotConfirmed(_))));
    let (missing, _) = invoke(
        &["audit", "prune", "--before", &cutoff, "--yes"],
        &BTreeMap::new(),
    )
    .await;
    assert!(matches!(
        missing,
        Err(CliError::Missing("TANDEM_PRIVILEGED_DATABASE_URL"))
    ));
    assert_eq!(audit_count(&db).await, 2);

    let app_role = minimal_environment(db.app_url(), &db.ca_file, "TANDEM_PRIVILEGED_DATABASE_URL");
    let (denied, _) = invoke(&["audit", "prune", "--before", &cutoff, "--yes"], &app_role).await;
    assert!(
        matches!(denied, Err(CliError::Database(_))),
        "the app role cannot prune"
    );
    assert_eq!(audit_count(&db).await, 2);

    let (result, output) = invoke(
        &["audit", "prune", "--before", &cutoff, "--yes"],
        &privileged,
    )
    .await;
    result.unwrap();
    assert!(output[0].starts_with("deleted 1 audit entry"), "{output:?}");
    assert_eq!(audit_count(&db).await, 1);
}
