#![expect(clippy::unwrap_used, clippy::panic)]

use super::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn bug_action(args: &[&str]) -> BugAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug { action } => action,
        _ => panic!("expected Commands::Bug"),
    }
}

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).unwrap()
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

/// Every verb under `bug` binds to its own `BugAction` variant. This pins the
/// subcommand wiring so a renamed or dropped verb fails here rather than only
/// surfacing through dispatch.
#[test]
fn each_verb_binds_to_its_action_variant() {
    assert!(matches!(
        bug_action(&["bzr", "bug", "list"]),
        BugAction::List(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "view", "1"]),
        BugAction::View(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "search", "q"]),
        BugAction::Search(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "history", "1"]),
        BugAction::History(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "create"]),
        BugAction::Create(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "my"]),
        BugAction::My(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "clone", "1"]),
        BugAction::Clone(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "update", "1"]),
        BugAction::Update(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "resolve", "1"]),
        BugAction::Resolve(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "close", "1"]),
        BugAction::Close(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "reopen", "1"]),
        BugAction::Reopen(_)
    ));
    assert!(matches!(
        bug_action(&["bzr", "bug", "dup", "1", "2"]),
        BugAction::Dup(_)
    ));
}

#[test]
fn dry_run_global_flag_applies_to_bug_mutation() {
    let cli = parse(&[
        "bzr",
        "--dry-run",
        "bug",
        "update",
        "1",
        "--status",
        "RESOLVED",
    ]);
    assert!(cli.dry_run);
    assert!(matches!(
        cli.command,
        Commands::Bug {
            action: BugAction::Update(_)
        }
    ));
}

#[test]
fn yes_global_flag_long_form_applies_to_batch_update() {
    let cli = parse(&[
        "bzr",
        "--yes",
        "bug",
        "update",
        "1",
        "2",
        "3",
        "--priority",
        "high",
    ]);
    assert!(cli.yes);
}

#[test]
fn yes_global_flag_short_form_applies_to_batch_update() {
    let cli = parse(&["bzr", "-y", "bug", "resolve", "1", "2", "3"]);
    assert!(cli.yes);
    assert!(matches!(
        cli.command,
        Commands::Bug {
            action: BugAction::Resolve(_)
        }
    ));
}

#[test]
fn global_flag_accepted_after_bug_action() {
    // Global flags are positionally flexible; `--dry-run` after the verb still
    // sets the flag and preserves the action shape.
    let cli = parse(&["bzr", "bug", "update", "1", "--dry-run"]);
    assert!(cli.dry_run);
    assert!(matches!(
        cli.command,
        Commands::Bug {
            action: BugAction::Update(_)
        }
    ));
}

#[test]
fn bug_requires_a_subcommand() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug"]),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn bug_rejects_unknown_subcommand() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "frobnicate"]),
        ErrorKind::InvalidSubcommand
    );
}
