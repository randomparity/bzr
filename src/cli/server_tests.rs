#![expect(clippy::unwrap_used, clippy::panic)]

use super::ServerAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn server_action(args: &[&str]) -> ServerAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Server { action } => action,
        _ => panic!("expected Commands::Server"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_server_info() {
    assert!(matches!(
        server_action(&["bzr", "server", "info"]),
        ServerAction::Info
    ));
}

#[test]
fn parse_server_info_rejects_positional() {
    assert_eq!(
        parse_error_kind(&["bzr", "server", "info", "extra"]),
        ErrorKind::UnknownArgument
    );
}

#[test]
fn parse_server_requires_action() {
    assert_eq!(
        parse_error_kind(&["bzr", "server"]),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn parse_server_rejects_unknown_action() {
    assert_eq!(
        parse_error_kind(&["bzr", "server", "status"]),
        ErrorKind::InvalidSubcommand
    );
}
