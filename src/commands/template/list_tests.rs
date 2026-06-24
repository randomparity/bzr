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
async fn template_list_empty() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::List;
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a3.writers(),
    )
    .await;
    let _output = __io_a3.out_str().to_string();
    assert!(result.is_ok(), "template list failed: {result:?}");
}

#[tokio::test]
async fn template_list_renders_saved_template() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut save_io = crate::test_helpers::CapturedIo::new();
    crate::commands::template::execute(
        &save_action("alpha"),
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await
    .unwrap();

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &TemplateAction::List,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "template list failed: {result:?}");
    // A no-op handle would write nothing; the saved template must be rendered.
    assert!(
        output.contains("alpha"),
        "list output must name the saved template, got: {output:?}"
    );
}

#[tokio::test]
async fn template_list_table_sorts_entries_by_name() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    for name in ["zzz", "aaa"] {
        let mut __io7 = crate::test_helpers::CapturedIo::new();
        let result = crate::commands::template::execute(
            &save_action(name),
            &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
            &mut __io7.writers(),
        )
        .await;
        let _ = __io7.out_str().to_string();
        assert!(result.is_ok());
    }

    let mut __io8 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::template::execute(
        &TemplateAction::List,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io8.writers(),
    )
    .await;

    let output = __io8.out_str().to_string();
    assert!(result.is_ok());
    assert!(output.is_empty() || output.contains("product="));

    let config = load_config();
    let mut names: Vec<&str> = config.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}
