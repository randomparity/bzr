#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

use crate::cli::{TemplateAction, TemplateFields};
use crate::config::Config;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn current_config_path() -> PathBuf {
    Config::path_at(None).unwrap()
}

fn load_config() -> Config {
    let path = current_config_path();
    Config::load_at(Some(&path)).unwrap()
}

fn save_action(name: &str) -> TemplateAction {
    TemplateAction::Save {
        name: name.into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            priority: Some("P1".into()),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn template_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Save a template
    let action = save_action("test-tmpl");
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let _output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    // Show the saved template
    let action = TemplateAction::Show {
        name: "test-tmpl".into(),
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "template show failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "test-tmpl");
    assert_eq!(parsed["product"], "TestProduct");
    assert_eq!(parsed["priority"], "P1");
}

#[tokio::test]
async fn template_save_requires_field() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "empty-tmpl".into(),
        fields: TemplateFields::default(),
    };
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
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
        fields: TemplateFields {
            version: Some("1.2.3".into()),
            ..Default::default()
        },
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    let _ = __io.out_str().to_string();
    assert!(
        result.is_ok(),
        "single-field template should save: {result:?}"
    );
}

#[tokio::test]
async fn template_save_and_show_create_metadata_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "routing".into(),
        fields: TemplateFields {
            url: Some("https://example.com/repro".into()),
            whiteboard: Some("needs-triage".into()),
            target_milestone: Some("M1".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cc@example.com".into()],
            keywords: vec!["regression".into()],
            groups: vec!["security".into()],
            flag: vec!["review?".into()],
            ..Default::default()
        },
    };
    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await;
    let _ = save_io.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    let mut show_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &TemplateAction::Show {
            name: "routing".into(),
        },
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut show_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "template show failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(show_io.out_str().trim()).unwrap();
    assert_eq!(parsed["url"], "https://example.com/repro");
    assert_eq!(parsed["whiteboard"], "needs-triage");
    assert_eq!(parsed["target_milestone"], "M1");
    assert_eq!(parsed["deadline"], "2026-12-31");
    assert_eq!(parsed["cc"], serde_json::json!(["cc@example.com"]));
    assert_eq!(parsed["keywords"], serde_json::json!(["regression"]));
    assert_eq!(parsed["groups"], serde_json::json!(["security"]));
    assert_eq!(parsed["flags"], serde_json::json!(["review?"]));
}

#[tokio::test]
async fn template_save_rejects_malformed_deadline() {
    let mut cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "bad-deadline".into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            deadline: Some("2026-99-99".into()),
            ..Default::default()
        },
    };
    let err = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut cap_io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--deadline"));
}

#[tokio::test]
async fn template_save_existing_entry_reports_updated_and_replaces_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &save_action("existing"),
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    assert!(result.is_ok());

    let update = TemplateAction::Save {
        name: "existing".into(),
        fields: TemplateFields {
            component: Some("Updated".into()),
            version: Some("123".into()),
            severity: Some("major".into()),
            description: Some("updated".into()),
            ..Default::default()
        },
    };
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &update,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a4.writers(),
    )
    .await;
    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "existing");
    assert_eq!(parsed["action"], "updated");

    let config = load_config();
    let saved = &config.templates["existing"];
    assert_eq!(saved.product, None);
    assert_eq!(saved.component.as_deref(), Some("Updated"));
    assert_eq!(saved.version.as_deref(), Some("123"));
    assert_eq!(saved.description.as_deref(), Some("updated"));
}
