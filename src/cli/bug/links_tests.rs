#![expect(clippy::unwrap_used)]

use clap::error::ErrorKind;
use clap::Parser;

use crate::cli::Cli;

fn parse(args: &[&str]) -> Result<(), clap::Error> {
    Cli::try_parse_from(args).map(|_| ())
}

#[test]
fn links_parses_minimal() {
    assert!(parse(&["bzr", "bug", "links", "42"]).is_ok());
}

#[test]
fn links_depth_requires_recursive() {
    let err = parse(&["bzr", "bug", "links", "42", "--depth", "2"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
}

#[test]
fn links_depth_out_of_range_rejected() {
    let low = parse(&["bzr", "bug", "links", "42", "--recursive", "--depth", "0"]).unwrap_err();
    assert_eq!(low.kind(), ErrorKind::ValueValidation);
    let high = parse(&["bzr", "bug", "links", "42", "--recursive", "--depth", "11"]).unwrap_err();
    assert_eq!(high.kind(), ErrorKind::ValueValidation);
}

#[test]
fn links_relation_invalid_rejected() {
    let err = parse(&["bzr", "bug", "links", "42", "--relation", "bogus"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
}
