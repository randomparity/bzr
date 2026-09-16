#![expect(clippy::unwrap_used)]

use std::path::Path;

use crate::cli::{TemplateAction, TemplateFields, TemplateUpdateArgs};
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

#[expect(clippy::too_many_arguments)]
fn update_action(
    name: &str,
    product: Option<&str>,
    component: Option<&str>,
    version: Option<&str>,
    priority: Option<&str>,
    severity: Option<&str>,
    clear: &[&str],
) -> TemplateAction {
    TemplateAction::Update(TemplateUpdateArgs {
        name: name.into(),
        fields: TemplateFields {
            product: product.map(Into::into),
            component: component.map(Into::into),
            version: version.map(Into::into),
            priority: priority.map(Into::into),
            severity: severity.map(Into::into),
            ..Default::default()
        },
        clear: clear.iter().map(|s| (*s).to_string()).collect(),
    })
}

async fn run(action: &TemplateAction, config_path: &Path) -> crate::error::Result<String> {
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.to_path_buf())),
        &mut io.writers(),
    )
    .await;
    result.map(|()| io.out_str().to_string())
}

#[tokio::test]
async fn template_update_merges_field() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap(); // product + component + priority

    run(
        &update_action("t", None, None, None, None, Some("blocker"), &[]),
        &config_path,
    )
    .await
    .unwrap();

    let config = load_config(&config_path);
    let t = &config.templates["t"];
    assert_eq!(t.severity.as_deref(), Some("blocker"));
    // Untouched fields are preserved.
    assert_eq!(t.product.as_deref(), Some("TestProduct"));
    assert_eq!(t.priority.as_deref(), Some("P1"));
}

#[tokio::test]
async fn template_update_clear_resets_field() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap();

    run(
        &update_action("t", None, None, None, None, None, &["priority"]),
        &config_path,
    )
    .await
    .unwrap();

    let config = load_config(&config_path);
    assert!(config.templates["t"].priority.is_none());
    assert_eq!(
        config.templates["t"].product.as_deref(),
        Some("TestProduct")
    );
}

#[tokio::test]
async fn template_update_merges_and_clears_create_metadata_fields() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("routing"), &config_path).await.unwrap();

    run(
        &TemplateAction::Update(TemplateUpdateArgs {
            name: "routing".into(),
            fields: TemplateFields {
                url: Some("https://example.com/updated".into()),
                whiteboard: Some("needs-routing".into()),
                target_milestone: Some("M2".into()),
                deadline: Some("2026-12-31".into()),
                cc: vec!["cc@example.com".into()],
                keywords: vec!["regression".into()],
                groups: vec!["security".into()],
                flag: vec!["review+".into()],
                ..Default::default()
            },
            clear: vec![],
        }),
        &config_path,
    )
    .await
    .unwrap();

    let config = load_config(&config_path);
    let t = &config.templates["routing"];
    assert_eq!(t.url.as_deref(), Some("https://example.com/updated"));
    assert_eq!(t.whiteboard.as_deref(), Some("needs-routing"));
    assert_eq!(t.target_milestone.as_deref(), Some("M2"));
    assert_eq!(t.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(t.cc, vec!["cc@example.com"]);
    assert_eq!(t.keywords, vec!["regression"]);
    assert_eq!(t.groups, vec!["security"]);
    assert_eq!(t.flags, vec!["review+"]);

    run(
        &TemplateAction::Update(TemplateUpdateArgs {
            name: "routing".into(),
            fields: TemplateFields::default(),
            clear: vec![
                "url".into(),
                "whiteboard".into(),
                "target-milestone".into(),
                "deadline".into(),
                "cc".into(),
                "keywords".into(),
                "groups".into(),
                "flags".into(),
            ],
        }),
        &config_path,
    )
    .await
    .unwrap();

    let config = load_config(&config_path);
    let t = &config.templates["routing"];
    assert!(t.url.is_none());
    assert!(t.whiteboard.is_none());
    assert!(t.target_milestone.is_none());
    assert!(t.deadline.is_none());
    assert!(t.cc.is_empty());
    assert!(t.keywords.is_empty());
    assert!(t.groups.is_empty());
    assert!(t.flags.is_empty());
    assert_eq!(t.product.as_deref(), Some("TestProduct"));
}

#[tokio::test]
async fn template_update_unknown_template_errors() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    let err = run(
        &update_action("missing", Some("X"), None, None, None, None, &[]),
        &config_path,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("template 'missing' not found"));
}

#[tokio::test]
async fn template_update_requires_a_change() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap();
    let err = run(
        &update_action("t", None, None, None, None, None, &[]),
        &config_path,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("no changes"));
}

#[tokio::test]
async fn template_update_unknown_clear_field_errors() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap();
    let err = run(
        &update_action("t", None, None, None, None, None, &["bogus"]),
        &config_path,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("unknown --clear field"));
}

#[tokio::test]
async fn template_update_clearing_all_fields_rejected() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap(); // product, component, priority set
    let err = run(
        &update_action(
            "t",
            None,
            None,
            None,
            None,
            None,
            &["product", "component", "priority"],
        ),
        &config_path,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("at least one field"));
}

#[tokio::test]
async fn template_update_clear_wins_over_set() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;
    run(&save_action("t"), &config_path).await.unwrap(); // product + component + priority

    // Set and clear the same field in one call: clear wins.
    let a = update_action("t", None, None, None, None, Some("blocker"), &["severity"]);
    run(&a, &config_path).await.unwrap();
    assert!(load_config(&config_path).templates["t"].severity.is_none());
}
