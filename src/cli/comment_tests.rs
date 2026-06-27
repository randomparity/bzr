#![expect(clippy::unwrap_used, clippy::panic)]

use super::CommentAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;
use std::path::PathBuf;

fn comment_action(args: &[&str]) -> CommentAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Comment { action } => action,
        _ => panic!("expected Commands::Comment"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_comment_list_minimal_defaults_since_none() {
    match comment_action(&["bzr", "comment", "list", "12345"]) {
        CommentAction::List { bug_id, since, .. } => {
            assert_eq!(bug_id, 12345);
            assert!(since.is_none());
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_comment_list_binds_since() {
    match comment_action(&["bzr", "comment", "list", "12345", "--since", "2026-01-01"]) {
        CommentAction::List { since, .. } => {
            assert_eq!(since.as_deref(), Some("2026-01-01"));
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_comment_list_rejects_non_numeric_bug_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "comment", "list", "abc"]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parse_comment_list_requires_bug_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "comment", "list"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_comment_add_minimal_defaults() {
    match comment_action(&["bzr", "comment", "add", "12345"]) {
        CommentAction::Add {
            bug_id,
            body,
            body_file,
            private,
        } => {
            assert_eq!(bug_id, 12345);
            assert!(body.is_none());
            assert!(body_file.is_none());
            assert!(!private);
        }
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_comment_add_binds_body_and_private() {
    match comment_action(&[
        "bzr",
        "comment",
        "add",
        "12345",
        "--body",
        "Reproduced",
        "--private",
    ]) {
        CommentAction::Add { body, private, .. } => {
            assert_eq!(body.as_deref(), Some("Reproduced"));
            assert!(private);
        }
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_comment_add_binds_body_file() {
    match comment_action(&["bzr", "comment", "add", "12345", "--body-file", "notes.txt"]) {
        CommentAction::Add { body_file, .. } => {
            assert_eq!(body_file, Some(PathBuf::from("notes.txt")));
        }
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_comment_add_rejects_body_with_body_file() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "comment",
            "add",
            "12345",
            "--body",
            "x",
            "--body-file",
            "notes.txt",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_comment_tag_collects_repeated_add_and_remove() {
    match comment_action(&[
        "bzr", "comment", "tag", "200", "--add", "useful", "--add", "tag2", "--remove", "spam",
    ]) {
        CommentAction::Tag {
            comment_id,
            add,
            remove,
        } => {
            assert_eq!(comment_id, 200);
            assert_eq!(add, vec!["useful".to_string(), "tag2".to_string()]);
            assert_eq!(remove, vec!["spam".to_string()]);
        }
        _ => panic!("expected Tag"),
    }
}

#[test]
fn parse_comment_tag_defaults_empty_vecs() {
    match comment_action(&["bzr", "comment", "tag", "200"]) {
        CommentAction::Tag { add, remove, .. } => {
            assert!(add.is_empty());
            assert!(remove.is_empty());
        }
        _ => panic!("expected Tag"),
    }
}

#[test]
fn parse_comment_search_tags_binds_query() {
    match comment_action(&["bzr", "comment", "search-tags", "spam"]) {
        CommentAction::SearchTags { query } => assert_eq!(query, "spam"),
        _ => panic!("expected SearchTags"),
    }
}

#[test]
fn parse_comment_search_tags_requires_query() {
    assert_eq!(
        parse_error_kind(&["bzr", "comment", "search-tags"]),
        ErrorKind::MissingRequiredArgument
    );
}
