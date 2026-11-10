use crate::{Cli, CliError, Command, check_prune_cutoff, parse_cutoff, require_confirmation};
use clap::Parser;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("tandem").chain(arguments.iter().copied()))
}

#[test]
fn every_command_parses() {
    assert!(matches!(
        parse(&["migrate", "up"]).unwrap().command,
        Command::Migrate(_)
    ));
    assert!(parse(&["migrate", "down"]).is_ok());
    assert!(parse(&["migrate", "down", "--yes"]).is_ok());
    assert!(parse(&["vault", "rekey"]).is_ok());
    assert!(parse(&["admin", "issue-reset", "--email", "ada@example.test"]).is_ok());
    assert!(parse(&["audit", "prune", "--before", "2020-01-01", "--yes"]).is_ok());
}

#[test]
fn bad_invocations_are_refused() {
    assert!(parse(&[]).is_err());
    assert!(parse(&["migrate"]).is_err());
    assert!(parse(&["admin", "issue-reset"]).is_err());
    assert!(parse(&["audit", "prune"]).is_err());
    assert!(parse(&["audit", "prune", "--before", "yesterday"]).is_err());
    assert!(parse(&["wipe"]).is_err());
}

#[test]
fn cutoffs_accept_timestamps_and_dates() {
    assert_eq!(
        parse_cutoff("2024-03-01T12:30:00Z").unwrap(),
        datetime!(2024-03-01 12:30:00 UTC)
    );
    assert_eq!(
        parse_cutoff("2024-03-01T12:30:00+13:00").unwrap(),
        datetime!(2024-02-29 23:30:00 UTC)
    );
    assert_eq!(
        parse_cutoff("2024-03-01").unwrap(),
        datetime!(2024-03-01 00:00:00 UTC)
    );
    assert!(parse_cutoff("01/03/2024").is_err());
    assert!(parse_cutoff("").is_err());
}

#[test]
fn a_cutoff_inside_ninety_days_is_refused() {
    let now = datetime!(2026-06-01 00:00:00 UTC);
    assert!(check_prune_cutoff(now, now).is_err());
    assert!(check_prune_cutoff(now - Duration::days(89), now).is_err());
    assert!(check_prune_cutoff(now - Duration::days(90) + Duration::seconds(1), now).is_err());
    assert!(check_prune_cutoff(now - Duration::days(90), now).is_ok());
    assert!(check_prune_cutoff(now - Duration::days(400), now).is_ok());
    assert!(check_prune_cutoff(OffsetDateTime::now_utc() + Duration::days(1), now).is_err());
}

#[test]
fn destructive_commands_need_yes() {
    let refused = require_confirmation(false, "this deletes things").unwrap_err();
    assert!(matches!(refused, CliError::NotConfirmed(_)));
    assert!(refused.to_string().contains("--yes"));
    assert!(!refused.to_string().contains('\n'));
    assert!(require_confirmation(true, "this deletes things").is_ok());
}
