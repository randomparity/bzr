#![expect(clippy::unwrap_used, clippy::panic)]

use super::{TemplateAction, UpdateArgs};
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn template_action(args: &[&str]) -> TemplateAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Template { action } => action,
        _ => panic!("expected Commands::Template"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_template_save_maps_routing_fields() {
    match template_action(&[
        "bzr",
        "template",
        "save",
        "sec",
        "--product",
        "Security",
        "--component",
        "Vulnerabilities",
        "--severity",
        "critical",
    ]) {
        TemplateAction::Save { name, fields } => {
            assert_eq!(name, "sec");
            assert_eq!(fields.product.as_deref(), Some("Security"));
            assert_eq!(fields.component.as_deref(), Some("Vulnerabilities"));
            assert_eq!(fields.severity.as_deref(), Some("critical"));
            assert!(fields.assignee.is_none());
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_template_save_maps_kebab_flags_to_snake_fields() {
    match template_action(&[
        "bzr",
        "template",
        "save",
        "t",
        "--op-sys",
        "Linux",
        "--rep-platform",
        "PC",
        "--target-milestone",
        "1.0",
    ]) {
        TemplateAction::Save { fields, .. } => {
            assert_eq!(fields.op_sys.as_deref(), Some("Linux"));
            assert_eq!(fields.rep_platform.as_deref(), Some("PC"));
            assert_eq!(fields.target_milestone.as_deref(), Some("1.0"));
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_template_save_splits_comma_delimited_lists() {
    match template_action(&[
        "bzr",
        "template",
        "save",
        "t",
        "--cc",
        "a@example.com,b@example.com",
        "--keywords",
        "regression,security",
        "--groups",
        "core,partners",
    ]) {
        TemplateAction::Save { fields, .. } => {
            assert_eq!(fields.cc, vec!["a@example.com", "b@example.com"]);
            assert_eq!(fields.keywords, vec!["regression", "security"]);
            assert_eq!(fields.groups, vec!["core", "partners"]);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_template_save_collects_repeated_flag() {
    match template_action(&[
        "bzr",
        "template",
        "save",
        "t",
        "--flag",
        "review?",
        "--flag",
        "needinfo?(me@example.com)",
    ]) {
        TemplateAction::Save { fields, .. } => {
            assert_eq!(fields.flag, vec!["review?", "needinfo?(me@example.com)"]);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_template_save_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "template", "save"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_template_update_merges_field_and_clear() {
    match template_action(&[
        "bzr",
        "template",
        "update",
        "sec",
        "--severity",
        "blocker",
        "--clear",
        "assignee",
        "--clear",
        "url",
    ]) {
        TemplateAction::Update(UpdateArgs {
            name,
            fields,
            clear,
        }) => {
            assert_eq!(name, "sec");
            assert_eq!(fields.severity.as_deref(), Some("blocker"));
            assert_eq!(clear, vec!["assignee", "url"]);
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_template_update_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "template", "update"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_template_list() {
    assert!(matches!(
        template_action(&["bzr", "template", "list"]),
        TemplateAction::List
    ));
}

#[test]
fn parse_template_show_binds_name() {
    match template_action(&["bzr", "template", "show", "sec"]) {
        TemplateAction::Show { name } => assert_eq!(name, "sec"),
        _ => panic!("expected Show"),
    }
}

#[test]
fn parse_template_delete_binds_name() {
    match template_action(&["bzr", "template", "delete", "sec"]) {
        TemplateAction::Delete { name } => assert_eq!(name, "sec"),
        _ => panic!("expected Delete"),
    }
}
