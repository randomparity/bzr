#![expect(clippy::unwrap_used, clippy::panic)]

use super::ClassificationAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

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
        ClassificationAction::List
    ));
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
        ClassificationAction::View { name } => assert_eq!(name, "Unclassified"),
        ClassificationAction::List => panic!("expected View"),
    }
}

#[test]
fn parse_classification_view_accepts_numeric_id_as_string() {
    match classification_action(&["bzr", "classification", "view", "1"]) {
        ClassificationAction::View { name } => assert_eq!(name, "1"),
        ClassificationAction::List => panic!("expected View"),
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
