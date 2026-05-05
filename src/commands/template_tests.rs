#![expect(clippy::unwrap_used)]

use crate::cli::TemplateAction;
use crate::config::Config;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

fn save_action(name: &str) -> TemplateAction {
    TemplateAction::Save {
        name: name.into(),
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        version: None,
        priority: Some("P1".into()),
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
    }
}

#[tokio::test]
async fn template_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Save a template
    let action = save_action("test-tmpl");
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "template save failed: {result:?}");

    // Show the saved template
    let action = TemplateAction::Show {
        name: "test-tmpl".into(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "template show failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["name"], "test-tmpl");
    assert_eq!(parsed["product"], "TestProduct");
    assert_eq!(parsed["priority"], "P1");
}

#[tokio::test]
async fn template_save_requires_field() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "empty-tmpl".into(),
        product: None,
        component: None,
        version: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "saving empty template should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least one field"),
        "expected validation error, got: {err}"
    );
}

#[tokio::test]
async fn template_save_with_single_field_succeeds() {
    // A single non-None field is enough to satisfy the
    // "at least one field required" validator.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "version-only".into(),
        product: None,
        component: None,
        version: Some("1.2.3".into()),
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
    };
    let (result, _) = capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(
        result.is_ok(),
        "single-field template should save: {result:?}"
    );
}

#[tokio::test]
async fn template_delete_unknown_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Delete {
        name: "nonexistent".into(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "deleting unknown template should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not-found error, got: {err}"
    );
}

#[tokio::test]
async fn template_list_empty() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::List;
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "template list failed: {result:?}");
}

#[tokio::test]
async fn template_save_existing_entry_reports_updated_and_replaces_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let (result, _) = capture_stdout(super::execute(
        &save_action("existing"),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let update = TemplateAction::Save {
        name: "existing".into(),
        product: None,
        component: Some("Updated".into()),
        version: Some("123".into()),
        priority: None,
        severity: Some("major".into()),
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("updated".into()),
    };
    let (result, output) =
        capture_stdout(super::execute(&update, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["name"], "existing");
    assert_eq!(parsed["action"], "updated");

    let config = Config::load().unwrap();
    let saved = &config.templates["existing"];
    assert_eq!(saved.product, None);
    assert_eq!(saved.component.as_deref(), Some("Updated"));
    assert_eq!(saved.version.as_deref(), Some("123"));
    assert_eq!(saved.description.as_deref(), Some("updated"));
}

#[tokio::test]
async fn template_delete_existing_removes_entry() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let (result, _) = capture_stdout(super::execute(
        &save_action("delete-me"),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let (result, output) = capture_stdout(super::execute(
        &TemplateAction::Delete {
            name: "delete-me".into(),
        },
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["name"], "delete-me");
    assert_eq!(parsed["action"], "deleted");
    assert!(!Config::load().unwrap().templates.contains_key("delete-me"));
}

#[tokio::test]
async fn template_delete_table_prints_deleted_message() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let (result, _) = capture_stdout(super::execute(
        &save_action("table-delete"),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let (result, _output) = capture_stdout(super::execute(
        &TemplateAction::Delete {
            name: "table-delete".into(),
        },
        None,
        OutputFormat::Table,
        None,
    ))
    .await;
    assert!(result.is_ok());
    assert!(!Config::load()
        .unwrap()
        .templates
        .contains_key("table-delete"));
}

#[tokio::test]
async fn template_list_table_sorts_entries_by_name() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    for name in ["zzz", "aaa"] {
        let (result, _) = capture_stdout(super::execute(
            &save_action(name),
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
    }

    let (result, output) = capture_stdout(super::execute(
        &TemplateAction::List,
        None,
        OutputFormat::Table,
        None,
    ))
    .await;
    assert!(result.is_ok());
    assert!(output.is_empty() || output.contains("product="));

    let config = Config::load().unwrap();
    let mut names: Vec<&str> = config.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}

#[tokio::test]
async fn template_show_unknown_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let err = super::execute(
        &TemplateAction::Show {
            name: "missing".into(),
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("template 'missing' not found"));
}
