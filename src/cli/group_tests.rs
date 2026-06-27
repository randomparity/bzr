#![expect(clippy::unwrap_used, clippy::panic)]

use super::GroupAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn group_action(args: &[&str]) -> GroupAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Group { action } => action,
        _ => panic!("expected Commands::Group"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_group_add_user_binds_group_and_user() {
    match group_action(&[
        "bzr",
        "group",
        "add-user",
        "--group",
        "editbugs",
        "--user",
        "alice@example.com",
    ]) {
        GroupAction::AddUser { group, user } => {
            assert_eq!(group, "editbugs");
            assert_eq!(user, "alice@example.com");
        }
        _ => panic!("expected AddUser"),
    }
}

#[test]
fn parse_group_add_user_requires_user() {
    assert_eq!(
        parse_error_kind(&["bzr", "group", "add-user", "--group", "editbugs"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_group_remove_user_binds_group_and_user() {
    match group_action(&[
        "bzr",
        "group",
        "remove-user",
        "--group",
        "admins",
        "--user",
        "bob",
    ]) {
        GroupAction::RemoveUser { group, user } => {
            assert_eq!(group, "admins");
            assert_eq!(user, "bob");
        }
        _ => panic!("expected RemoveUser"),
    }
}

#[test]
fn parse_group_list_users_defaults_details_false() {
    match group_action(&["bzr", "group", "list-users", "--group", "editbugs"]) {
        GroupAction::ListUsers { group, details, .. } => {
            assert_eq!(group, "editbugs");
            assert!(!details);
        }
        _ => panic!("expected ListUsers"),
    }
}

#[test]
fn parse_group_list_users_binds_details_flag() {
    match group_action(&[
        "bzr",
        "group",
        "list-users",
        "--group",
        "editbugs",
        "--details",
    ]) {
        GroupAction::ListUsers { details, .. } => assert!(details),
        _ => panic!("expected ListUsers"),
    }
}

#[test]
fn parse_group_view_binds_positional() {
    match group_action(&["bzr", "group", "view", "editbugs"]) {
        GroupAction::View { group } => assert_eq!(group, "editbugs"),
        _ => panic!("expected View"),
    }
}

#[test]
fn parse_group_view_requires_group() {
    assert_eq!(
        parse_error_kind(&["bzr", "group", "view"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_group_create_minimal_binds_required() {
    match group_action(&[
        "bzr",
        "group",
        "create",
        "--name",
        "release-eng",
        "--description",
        "Release Engineering",
    ]) {
        GroupAction::Create {
            from_json,
            name,
            description,
            is_active,
        } => {
            assert!(from_json.is_none());
            assert_eq!(name.as_deref(), Some("release-eng"));
            assert_eq!(description.as_deref(), Some("Release Engineering"));
            assert!(is_active.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_group_create_requires_description_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "group", "create", "--name", "temp"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_group_create_from_json_relaxes_required_fields() {
    match group_action(&["bzr", "group", "create", "--from-json", "-"]) {
        GroupAction::Create {
            from_json,
            name,
            description,
            ..
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(name.is_none());
            assert!(description.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_group_create_binds_is_active_bool() {
    match group_action(&[
        "bzr",
        "group",
        "create",
        "--name",
        "temp",
        "--description",
        "Temporary",
        "--is-active",
        "false",
    ]) {
        GroupAction::Create { is_active, .. } => assert_eq!(is_active, Some(false)),
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_group_update_binds_group_positional_and_flags() {
    match group_action(&[
        "bzr",
        "group",
        "update",
        "editbugs",
        "--description",
        "Bug-edit privileges",
        "--is-active",
        "true",
    ]) {
        GroupAction::Update {
            group,
            description,
            is_active,
            ..
        } => {
            assert_eq!(group.as_deref(), Some("editbugs"));
            assert_eq!(description.as_deref(), Some("Bug-edit privileges"));
            assert_eq!(is_active, Some(true));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_group_update_requires_group_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "group", "update", "--is-active", "false"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_group_update_from_json_relaxes_group() {
    match group_action(&["bzr", "group", "update", "--from-json", "-"]) {
        GroupAction::Update {
            from_json, group, ..
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(group.is_none());
        }
        _ => panic!("expected Update"),
    }
}
