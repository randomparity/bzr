#![expect(clippy::unwrap_used, clippy::panic)]

use super::ComponentAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn component_action(args: &[&str]) -> ComponentAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Component { action } => action,
        _ => panic!("expected Commands::Component"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_component_list_binds_product() {
    match component_action(&["bzr", "component", "list", "--product", "MyApp"]) {
        ComponentAction::List { product } => assert_eq!(product, "MyApp"),
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_component_list_requires_product() {
    assert_eq!(
        parse_error_kind(&["bzr", "component", "list"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_component_view_binds_two_positionals() {
    match component_action(&["bzr", "component", "view", "MyApp", "Backend"]) {
        ComponentAction::View { product, name } => {
            assert_eq!(product, "MyApp");
            assert_eq!(name, "Backend");
        }
        _ => panic!("expected View"),
    }
}

#[test]
fn parse_component_view_requires_both_positionals() {
    assert_eq!(
        parse_error_kind(&["bzr", "component", "view", "MyApp"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_component_create_minimal_binds_all_required() {
    match component_action(&[
        "bzr",
        "component",
        "create",
        "--product",
        "MyApp",
        "--name",
        "Backend",
        "--description",
        "Backend services",
        "--default-assignee",
        "dev@example.com",
    ]) {
        ComponentAction::Create {
            from_json,
            product,
            name,
            description,
            default_assignee,
        } => {
            assert!(from_json.is_none());
            assert_eq!(product.as_deref(), Some("MyApp"));
            assert_eq!(name.as_deref(), Some("Backend"));
            assert_eq!(description.as_deref(), Some("Backend services"));
            assert_eq!(default_assignee.as_deref(), Some("dev@example.com"));
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_component_create_requires_default_assignee_without_from_json() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "component",
            "create",
            "--product",
            "MyApp",
            "--name",
            "Backend",
            "--description",
            "d",
        ]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_component_create_from_json_relaxes_required_fields() {
    match component_action(&["bzr", "component", "create", "--from-json", "-"]) {
        ComponentAction::Create {
            from_json,
            product,
            name,
            description,
            default_assignee,
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(product.is_none());
            assert!(name.is_none());
            assert!(description.is_none());
            assert!(default_assignee.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_component_update_binds_id_positional() {
    match component_action(&[
        "bzr",
        "component",
        "update",
        "42",
        "--description",
        "Updated description",
    ]) {
        ComponentAction::Update {
            id, description, ..
        } => {
            assert_eq!(id, Some(42));
            assert_eq!(description.as_deref(), Some("Updated description"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_component_update_binds_name_based_targeting() {
    match component_action(&[
        "bzr",
        "component",
        "update",
        "--product",
        "MyApp",
        "--component",
        "Backend",
        "--name",
        "Core",
    ]) {
        ComponentAction::Update {
            id,
            product,
            component,
            name,
            ..
        } => {
            assert!(id.is_none());
            assert_eq!(product.as_deref(), Some("MyApp"));
            assert_eq!(component.as_deref(), Some("Backend"));
            assert_eq!(name.as_deref(), Some("Core"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_component_update_rejects_non_numeric_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "component", "update", "Backend"]),
        ErrorKind::ValueValidation
    );
}
