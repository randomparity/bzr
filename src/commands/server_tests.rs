#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ServerAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn server_info_returns_version_and_extensions() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"extensions": {}})),
        )
        .mount(&mock)
        .await;

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &ServerAction::Info,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["version"], "5.0.4");
}

#[tokio::test]
async fn server_capabilities_outputs_documented_shape() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"values": [
                {"name": "NEW", "can_change_to": [{"name": "ASSIGNED"}]}
            ]}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"fields": []})))
        .mount(&mock)
        .await;

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &ServerAction::Capabilities,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["version"], "5.0.4");
    assert_eq!(parsed["api_modes"][0], "rest");
    assert_eq!(parsed["status_transitions"][0]["from"], "NEW");
    assert!(parsed
        .as_object()
        .unwrap()
        .contains_key("max_attachment_size"));
    assert_eq!(parsed["supports_comments"], true);
}

#[tokio::test]
async fn server_info_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let result = super::execute(
        &ServerAction::Info,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}
