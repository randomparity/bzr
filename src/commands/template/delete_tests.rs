#![expect(clippy::unwrap_used)]

use std::path::Path;

use crate::cli::{TemplateAction, TemplateFields};
use crate::config::Config;
use crate::test_helpers::setup_isolated_env;
use crate::types::OutputFormat;

fn load_config(config_path: &Path) -> Config {
    Config::load_at(Some(config_path)).unwrap()
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
async fn template_delete_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let action = TemplateAction::Delete {
        name: "nonexistent".into(),
    };
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "deleting unknown template should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not-found error, got: {err}"
    );
}

#[tokio::test]
async fn template_delete_existing_removes_entry() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &save_action("delete-me"),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io3.writers(),
    )
    .await;

    let _ = __io3.out_str().to_string();
    assert!(result.is_ok());

    let mut __io4 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &TemplateAction::Delete {
            name: "delete-me".into(),
        },
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io4.writers(),
    )
    .await;

    let output = __io4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "delete-me");
    assert_eq!(parsed["action"], "deleted");
    assert!(!load_config(&config_path)
        .templates
        .contains_key("delete-me"));
}

#[tokio::test]
async fn template_delete_json_matches_domain_mutation_output() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &save_action("delete-json-shape"),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut save_io.writers(),
    )
    .await;
    let _ = save_io.out_str().to_string();
    assert!(result.is_ok());

    let mut delete_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &TemplateAction::Delete {
            name: "delete-json-shape".into(),
        },
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut delete_io.writers(),
    )
    .await;

    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(delete_io.out_str());
    assert_eq!(
        parsed,
        serde_json::json!({"name": "delete-json-shape", "action": "deleted"})
    );
}

#[tokio::test]
async fn template_delete_table_prints_deleted_message() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let mut __io5 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &save_action("table-delete"),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io5.writers(),
    )
    .await;

    let _ = __io5.out_str().to_string();
    assert!(result.is_ok());

    let mut __io6 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &TemplateAction::Delete {
            name: "table-delete".into(),
        },
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io6.writers(),
    )
    .await;

    let _output = __io6.out_str().to_string();
    assert!(result.is_ok());
    assert!(!load_config(&config_path)
        .templates
        .contains_key("table-delete"));
}
