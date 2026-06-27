#![expect(clippy::unwrap_used, clippy::panic)]

use super::UserAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn user_action(args: &[&str]) -> UserAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::User { action } => action,
        _ => panic!("expected Commands::User"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_user_search_minimal_defaults_details_false() {
    match user_action(&["bzr", "user", "search", "alice"]) {
        UserAction::Search { query, details, .. } => {
            assert_eq!(query, "alice");
            assert!(!details);
        }
        _ => panic!("expected Search"),
    }
}

#[test]
fn parse_user_search_binds_details_flag() {
    match user_action(&["bzr", "user", "search", "alice", "--details"]) {
        UserAction::Search { details, .. } => assert!(details),
        _ => panic!("expected Search"),
    }
}

#[test]
fn parse_user_search_requires_query() {
    assert_eq!(
        parse_error_kind(&["bzr", "user", "search"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_user_create_minimal_defaults() {
    match user_action(&["bzr", "user", "create", "--email", "alice@example.com"]) {
        UserAction::Create {
            from_json,
            email,
            login,
            full_name,
            password,
        } => {
            assert!(from_json.is_none());
            assert_eq!(email.as_deref(), Some("alice@example.com"));
            assert!(login.is_none());
            assert!(full_name.is_none());
            assert!(password.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_user_create_binds_all_fields() {
    match user_action(&[
        "bzr",
        "user",
        "create",
        "--email",
        "bob@example.com",
        "--login",
        "bob",
        "--full-name",
        "Bob",
        "--password",
        "s3cret",
    ]) {
        UserAction::Create {
            login,
            full_name,
            password,
            ..
        } => {
            assert_eq!(login.as_deref(), Some("bob"));
            assert_eq!(full_name.as_deref(), Some("Bob"));
            assert_eq!(password.as_deref(), Some("s3cret"));
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_user_create_requires_email_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "user", "create", "--login", "bob"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_user_create_from_json_relaxes_email() {
    match user_action(&["bzr", "user", "create", "--from-json", "-"]) {
        UserAction::Create {
            from_json, email, ..
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(email.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_user_update_binds_user_and_disable_login() {
    match user_action(&[
        "bzr",
        "user",
        "update",
        "alice@example.com",
        "--disable-login",
        "true",
        "--login-denied-text",
        "Account closed",
    ]) {
        UserAction::Update {
            user,
            disable_login,
            login_denied_text,
            ..
        } => {
            assert_eq!(user.as_deref(), Some("alice@example.com"));
            assert_eq!(disable_login, Some(true));
            assert_eq!(login_denied_text.as_deref(), Some("Account closed"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_user_update_rejects_invalid_disable_login() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "user",
            "update",
            "alice@example.com",
            "--disable-login",
            "sometimes",
        ]),
        ErrorKind::InvalidValue
    );
}

#[test]
fn parse_user_update_requires_user_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "user", "update", "--real-name", "Alice"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_user_update_from_json_relaxes_user() {
    match user_action(&["bzr", "user", "update", "--from-json", "-"]) {
        UserAction::Update {
            from_json, user, ..
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(user.is_none());
        }
        _ => panic!("expected Update"),
    }
}
