#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, TemplateAction};
use crate::error::BzrError;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

fn create_action() -> BugAction {
    BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: "New bug".into(),
        version: Some("unspecified".into()),
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    }
}

#[tokio::test]
async fn bug_create_sends_post() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 99})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &create_action(),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn bug_create_missing_product_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: None,
        component: Some("General".into()),
        summary: "Needs product".into(),
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(msg) if msg.contains("--product")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_missing_component_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: None,
        summary: "Needs component".into(),
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(msg) if msg.contains("--component")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_with_unknown_template_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: Some("does-not-exist".into()),
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: "Bad template".into(),
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::Config(msg) if msg.contains("does-not-exist")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_with_template_fills_missing_fields() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Pre-populate a template with product/component/version so the
    // bug create command can resolve them from the template.
    let save = TemplateAction::Save {
        name: "tpl".into(),
        product: Some("TplProduct".into()),
        component: Some("TplComponent".into()),
        version: Some("9.9".into()),
        priority: Some("P2".into()),
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("from template".into()),
    };
    let (result, _) = capture_stdout(crate::commands::template::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "template save failed: {result:?}");

    // The mock should see the template's product/component/version
    // forwarded into the POST body.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("TplProduct"))
        .and(body_string_contains("TplComponent"))
        .and(body_string_contains("9.9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Create {
        template: Some("tpl".into()),
        product: None,
        component: None,
        summary: "From template".into(),
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(
        result.is_ok(),
        "bug create with template failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["id"], 7);
    assert_eq!(parsed["action"], "created");
}
