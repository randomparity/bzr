#![expect(clippy::unwrap_used, clippy::panic)]

use super::FieldAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn field_action(args: &[&str]) -> FieldAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Field { action } => action,
        _ => panic!("expected Commands::Field"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_field_aliases() {
    assert!(matches!(
        field_action(&["bzr", "field", "aliases"]),
        FieldAction::Aliases
    ));
}

#[test]
fn parse_field_aliases_rejects_positional() {
    assert_eq!(
        parse_error_kind(&["bzr", "field", "aliases", "status"]),
        ErrorKind::UnknownArgument
    );
}

#[test]
fn parse_field_list_binds_name() {
    match field_action(&["bzr", "field", "list", "status"]) {
        FieldAction::List { name, .. } => assert_eq!(name.as_deref(), Some("status")),
        FieldAction::Aliases => panic!("expected List"),
    }
}

#[test]
fn parse_field_list_preserves_canonical_name() {
    match field_action(&["bzr", "field", "list", "bug_severity"]) {
        FieldAction::List { name, .. } => assert_eq!(name.as_deref(), Some("bug_severity")),
        FieldAction::Aliases => panic!("expected List"),
    }
}

/// The no-argument form is the field-name listing (issue #718), so the missing
/// positional that used to be a usage error is now a valid invocation.
#[test]
fn parse_field_list_without_a_name_is_the_listing_form() {
    match field_action(&["bzr", "field", "list"]) {
        FieldAction::List { name, .. } => assert_eq!(name, None),
        FieldAction::Aliases => panic!("expected List"),
    }
}

#[test]
fn parse_field_rejects_unknown_action() {
    assert_eq!(
        parse_error_kind(&["bzr", "field", "values"]),
        ErrorKind::InvalidSubcommand
    );
}
