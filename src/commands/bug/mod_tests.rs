#![expect(clippy::unwrap_used)]

//! Tests for `commands/bug/mod.rs`: command capabilities and `bug_column_spec`
//! (via `execute`-level field-validation behavior).

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

// ---- capabilities ----
// Kill the `-> bool with true` mutant: read-only actions must return false.
// If the mutant replaces the body with `true`, these assertions fail.

#[test]
fn capabilities_are_anonymous_for_list() {
    let action = BugAction::List(crate::cli::ListArgs::default());
    let capabilities = super::capabilities(&action);
    assert!(!capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), None);
}

#[test]
fn capabilities_require_credentials_for_my() {
    let action = BugAction::My(crate::cli::MyArgs::default());
    let capabilities = super::capabilities(&action);
    assert!(!capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), Some("bug my"));
}

#[test]
fn capabilities_are_anonymous_for_search() {
    let action = BugAction::Search(crate::cli::SearchArgs {
        page_args: crate::cli::PageArgs::default(),
        query: Some("q".into()),
        from_url: None,
        save_as: None,
        limit: None,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let capabilities = super::capabilities(&action);
    assert!(!capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), None);
}

#[test]
fn capabilities_are_anonymous_for_view() {
    let action = BugAction::View(crate::cli::ViewArgs {
        ids: vec!["1".into()],
        permissive: false,
        web: false,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
    });
    let capabilities = super::capabilities(&action);
    assert!(!capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), None);
}

#[test]
fn capabilities_allow_dry_run_and_credentials_for_update() {
    let action = BugAction::Update(crate::cli::UpdateArgs::default());
    let capabilities = super::capabilities(&action);
    assert!(capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), Some("bug update"));
}

// ---- bug_column_spec — BugAction::My arm ----
// Kill the `delete match arm BugAction::My(a)` mutant.
// When My is missing, bug_column_spec returns None → field validation is
// skipped → an invalid `--fields` value would NOT be rejected. Here we
// give My an entirely non-existent field name and expect a validation error.

#[tokio::test]
async fn bug_column_spec_my_validates_unknown_field_in_table_mode() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::My(crate::cli::MyArgs {
        field_args: crate::cli::FieldArgs {
            fields: Some("totally_unknown_field_xyz".into()),
            exclude_fields: None,
        },
        ..Default::default()
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;

    // With a real My arm, validate_table_columns rejects the unknown field (exit 7).
    // With the mutant (arm deleted), it would skip validation and try to connect
    // to the server (which may fail differently, but not with InputValidation exit 7).
    assert!(
        result.is_err(),
        "My with an unknown --fields should fail field validation"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "InputValidation exit code expected, got: {err}"
    );
}

// ---- bug_column_spec — BugAction::Search arm ----
// Kill the `delete match arm BugAction::Search(a)` mutant.
// Same strategy: an all-unknown --fields on Search must exit 7 for field
// validation, not proceed to network I/O.

#[tokio::test]
async fn bug_column_spec_search_validates_unknown_field_in_table_mode() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Search(crate::cli::SearchArgs {
        page_args: crate::cli::PageArgs::default(),
        query: Some("crash".into()),
        from_url: None,
        save_as: None,
        limit: None,
        field_args: crate::cli::FieldArgs {
            fields: Some("totally_unknown_field_xyz".into()),
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_err(),
        "Search with an unknown --fields should fail field validation"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "InputValidation exit code expected, got: {err}"
    );
}
