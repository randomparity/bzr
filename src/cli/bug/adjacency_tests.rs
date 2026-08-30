#![expect(clippy::unwrap_used, clippy::panic)]

use crate::cli::{BugAction, Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

#[test]
fn accepts_mixed_ids_and_aliases() {
    let cli = Cli::try_parse_from(["bzr", "bug", "adjacency", "00123", "release/2026", "999999"])
        .unwrap();
    let Commands::Bug {
        action: BugAction::Adjacency(args),
    } = cli.command
    else {
        panic!("expected bug adjacency");
    };
    assert_eq!(args.ids, ["00123", "release/2026", "999999"]);
}

#[test]
fn requires_one_positional_request() {
    let error = Cli::try_parse_from(["bzr", "bug", "adjacency"])
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
}
