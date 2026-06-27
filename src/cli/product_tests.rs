#![expect(clippy::unwrap_used, clippy::panic)]

use super::ProductAction;
use crate::cli::{Cli, Commands};
use crate::types::product::ProductListType;
use clap::error::ErrorKind;
use clap::Parser as _;

fn product_action(args: &[&str]) -> ProductAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Product { action } => action,
        _ => panic!("expected Commands::Product"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_product_list_defaults_to_accessible() {
    match product_action(&["bzr", "product", "list"]) {
        ProductAction::List { r#type, .. } => assert_eq!(r#type, ProductListType::Accessible),
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_product_list_binds_selectable_type() {
    match product_action(&["bzr", "product", "list", "--type", "selectable"]) {
        ProductAction::List { r#type, .. } => assert_eq!(r#type, ProductListType::Selectable),
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_product_list_rejects_invalid_type() {
    assert_eq!(
        parse_error_kind(&["bzr", "product", "list", "--type", "bogus"]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parse_product_view_binds_name() {
    match product_action(&["bzr", "product", "view", "Firefox"]) {
        ProductAction::View { name, .. } => assert_eq!(name, "Firefox"),
        _ => panic!("expected View"),
    }
}

#[test]
fn parse_product_view_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "product", "view"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_product_create_minimal_defaults() {
    match product_action(&[
        "bzr",
        "product",
        "create",
        "--name",
        "MyApp",
        "--description",
        "My Application",
    ]) {
        ProductAction::Create {
            from_json,
            name,
            description,
            version,
            is_open,
        } => {
            assert!(from_json.is_none());
            assert_eq!(name.as_deref(), Some("MyApp"));
            assert_eq!(description.as_deref(), Some("My Application"));
            assert!(version.is_none());
            assert!(is_open.is_none());
        }
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_product_create_requires_name_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "product", "create", "--description", "d"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_product_create_requires_description_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "product", "create", "--name", "MyApp"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_product_create_from_json_relaxes_required_fields() {
    match product_action(&["bzr", "product", "create", "--from-json", "-"]) {
        ProductAction::Create {
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
fn parse_product_create_binds_is_open_bool() {
    match product_action(&[
        "bzr",
        "product",
        "create",
        "--name",
        "Legacy",
        "--description",
        "Archived",
        "--is-open",
        "false",
    ]) {
        ProductAction::Create { is_open, .. } => assert_eq!(is_open, Some(false)),
        _ => panic!("expected Create"),
    }
}

#[test]
fn parse_product_create_rejects_invalid_is_open() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "product",
            "create",
            "--name",
            "X",
            "--description",
            "d",
            "--is-open",
            "maybe",
        ]),
        ErrorKind::InvalidValue
    );
}

#[test]
fn parse_product_update_binds_name_positional_and_flags() {
    match product_action(&[
        "bzr",
        "product",
        "update",
        "Firefox",
        "--description",
        "Web browser",
        "--default-milestone",
        "v2.0",
    ]) {
        ProductAction::Update {
            name,
            description,
            default_milestone,
            ..
        } => {
            assert_eq!(name.as_deref(), Some("Firefox"));
            assert_eq!(description.as_deref(), Some("Web browser"));
            assert_eq!(default_milestone.as_deref(), Some("v2.0"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_product_update_requires_name_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "product", "update", "--description", "d"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_product_update_from_json_relaxes_name() {
    match product_action(&["bzr", "product", "update", "--from-json", "-"]) {
        ProductAction::Update {
            from_json, name, ..
        } => {
            assert_eq!(from_json.as_deref(), Some("-"));
            assert!(name.is_none());
        }
        _ => panic!("expected Update"),
    }
}
