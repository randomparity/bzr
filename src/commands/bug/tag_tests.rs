#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, TagArgs};
use crate::test_helpers::setup_test_env;
use crate::types::{ApiMode, OutputFormat};

#[tokio::test]
async fn bug_tag_uses_xmlrpc_and_reports_requested_changes() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let response = r#"<?xml version="1.0"?><methodResponse><params><param><value><struct/></value></param></params></methodResponse>"#;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains(
            "<methodName>Bug.update_tags</methodName>",
        ))
        .and(body_string_contains("<name>add</name>"))
        .and(body_string_contains("triage"))
        .and(body_string_contains("<name>remove</name>"))
        .and(body_string_contains("stale"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(response, "text/xml"))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Tag(TagArgs {
        id: 42,
        add: vec!["triage".into()],
        remove: vec!["stale".into()],
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            OutputFormat::Json,
            Some(ApiMode::XmlRpc),
        ),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "bug tag failed: {result:?}");
    let output: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(output["bug_id"], 42);
    assert_eq!(output["added"], serde_json::json!(["triage"]));
    assert_eq!(output["removed"], serde_json::json!(["stale"]));
}

#[tokio::test]
async fn bug_tag_without_changes_is_rejected_before_connecting() {
    let action = BugAction::Tag(TagArgs {
        id: 42,
        add: vec![],
        remove: vec![],
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let error = result.unwrap_err();
    assert_eq!(error.exit_code(), 7);
    assert!(error.to_string().contains("no bug tag changes"));
}
