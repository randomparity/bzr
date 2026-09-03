#![expect(clippy::unwrap_used, clippy::panic)]

use super::ClassificationAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::{CommandFactory as _, Parser as _};

fn classification_action(args: &[&str]) -> ClassificationAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Classification { action } => action,
        _ => panic!("expected Commands::Classification"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_classification_list() {
    assert!(matches!(
        classification_action(&["bzr", "classification", "list"]),
        ClassificationAction::List { .. }
    ));
}

#[test]
fn classification_help_describes_disabled_stream_behavior() {
    let mut command = Cli::command();
    let classification = command
        .find_subcommand_mut("classification")
        .unwrap_or_else(|| panic!("classification subcommand must exist"));
    let list = classification
        .find_subcommand_mut("list")
        .unwrap_or_else(|| panic!("classification list subcommand must exist"));
    let list_help = list.render_long_help().to_string();

    assert!(list_help.contains("API error 900"), "{list_help}");
    assert!(list_help.contains("stdout in table mode"), "{list_help}");
    assert!(list_help.contains("JSON writes"), "{list_help}");
    assert!(list_help.contains("an empty collection"), "{list_help}");
    assert!(
        list_help.contains("NDJSON emits no stdout records"),
        "{list_help}"
    );
    assert!(list_help.contains("note on stderr."), "{list_help}");
    assert!(
        list_help.contains("successfully fetched lone \"Unclassified\" row")
            && list_help.contains("preserved, with the note"),
        "{list_help}"
    );

    let top_help = classification.render_long_help().to_string();
    assert!(top_help.contains("API error 900"), "{top_help}");
    assert!(top_help.contains("writes the note to stdout"), "{top_help}");
    assert!(
        top_help.contains("NDJSON emits no stdout records"),
        "{top_help}"
    );
    assert!(
        top_help.contains("fetched \"Unclassified\" row is preserved")
            || (top_help.contains("fetched \"Unclassified\" row is")
                && top_help.contains("preserved and accompanied")),
        "{top_help}"
    );
}

#[test]
fn parse_classification_list_rejects_positional() {
    assert_eq!(
        parse_error_kind(&["bzr", "classification", "list", "extra"]),
        ErrorKind::UnknownArgument
    );
}

#[test]
fn parse_classification_view_binds_name() {
    match classification_action(&["bzr", "classification", "view", "Unclassified"]) {
        ClassificationAction::View { name, .. } => assert_eq!(name, "Unclassified"),
        ClassificationAction::List { .. } => panic!("expected View"),
    }
}

#[test]
fn parse_classification_view_accepts_numeric_id_as_string() {
    match classification_action(&["bzr", "classification", "view", "1"]) {
        ClassificationAction::View { name, .. } => assert_eq!(name, "1"),
        ClassificationAction::List { .. } => panic!("expected View"),
    }
}

#[test]
fn parse_classification_view_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "classification", "view"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_classification_rejects_unknown_action() {
    assert_eq!(
        parse_error_kind(&["bzr", "classification", "create"]),
        ErrorKind::InvalidSubcommand
    );
}
