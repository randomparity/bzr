#![expect(clippy::unwrap_used)]

use super::{validate, UpdateArgs};
use crate::cli::ComponentAction;
use crate::types::OutputFormat;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn args<'a>(
    description: Option<&'a str>,
    default_assignee: Option<&'a str>,
    is_active: Option<bool>,
) -> UpdateArgs<'a> {
    UpdateArgs {
        product: "Widget",
        component: "Core",
        description,
        default_assignee,
        is_active,
    }
}

#[test]
fn update_requires_one_mutable_field() {
    let error = validate(&args(None, None, None)).unwrap_err();
    assert!(error.to_string().contains("no fields to update"));
}

#[test]
fn update_allows_false_active_value() {
    assert!(validate(&args(None, None, Some(false))).is_ok());
}

#[test]
fn update_rejects_blank_target() {
    let mut input = args(Some("changed"), None, None);
    input.component = " ";
    let error = validate(&input).unwrap_err();
    assert!(error.to_string().contains("--component must not be empty"));
}

fn action() -> ComponentAction {
    ComponentAction::Update {
        product: "Widget".into(),
        component: "Core".into(),
        description: Some("Updated description".into()),
        default_assignee: Some("owner@example.test".into()),
        is_active: Some(false),
    }
}

#[tokio::test]
async fn component_update_dry_run_does_not_connect() {
    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::component::execute(
        &action(),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let output = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(output["resource"], "component");
    assert_eq!(output["action"], "dry-run");
    assert_eq!(output["changes"]["is_active"], false);
}

#[tokio::test]
async fn component_update_rejects_login_tokens_before_xmlrpc_dispatch() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        &format!(
            "default_server = \"token\"\n\n[servers.token]\nurl = \"{}\"\ntoken = \"login-token\"\n",
            mock.uri()
        ),
    );
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let error = crate::commands::component::execute(
        &action(),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path)),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert_eq!(error.exit_code(), 9);
    assert!(error.to_string().contains("requires an API key"));
}

#[tokio::test]
async fn component_update_uses_capability_gated_xmlrpc_dispatch() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        &format!(
            "default_server = \"rhbz\"\n\n[servers.rhbz]\nurl = \"{}\"\napi_key = \"test-key\"\nemail = \"test@example.com\"\n",
            mock.uri()
        ),
    );
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"extensions": {"RedHat": {}}})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<methodName>Component.update</methodName>"))
        .and(body_string_contains("<name>default_assignee</name>"))
        .and(body_string_contains("<name>is_active</name>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0"?><methodResponse><params><param><value><struct/></value></param></params></methodResponse>"#,
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::component::execute(
        &action(),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path)),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let output = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(output["action"], "updated");
    assert_eq!(output["component"], "Core");
}
