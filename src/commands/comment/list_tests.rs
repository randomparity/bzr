#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{CommentAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_one_comment(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "42": { "comments": [{
                "id": 1, "bug_id": 42, "text": "Hello world",
                "creator": "user@test.com", "creation_time": "2025-01-01T00:00:00Z",
                "is_private": false, "count": 0
            }]}}
        })))
        .mount(mock)
        .await;
}

fn list_with(projection: ProjectionArgs) -> CommentAction {
    CommentAction::List {
        bug_id: 42,
        since: None,
        projection,
    }
}

#[tokio::test]
async fn comment_list_returns_comments() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [{
                        "id": 1,
                        "bug_id": 42,
                        "text": "Hello world",
                        "creator": "user@test.com",
                        "creation_time": "2025-01-01T00:00:00Z",
                        "is_private": false,
                        "count": 0
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_id: 42,
        since: None,
        projection: crate::cli::ProjectionArgs::default(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["text"], "Hello world");
    assert_eq!(parsed[0]["creator"], "user@test.com");
}

#[tokio::test]
async fn comment_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_id: 42,
        since: None,
        projection: crate::cli::ProjectionArgs::default(),
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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

#[tokio::test]
async fn comment_list_rejects_malformed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = CommentAction::List {
        bug_id: 42,
        since: Some("nope".into()),
        projection: crate::cli::ProjectionArgs::default(),
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(msg.contains("--since"), "error should name the flag: {msg}");
    assert!(
        msg.contains("nope"),
        "error should echo the offending input: {msg}"
    );
}

#[tokio::test]
async fn comment_list_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("id".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed[0]["id"], 1);
    assert!(
        parsed[0].get("text").is_none(),
        "text should be projected out"
    );
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn comment_list_ndjson_fields_projects_each_line() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("creator".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            OutputFormat::Ndjson,
            None,
        ),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(io.out_str().trim(), r#"{"creator":"user@test.com"}"#);
}

#[tokio::test]
async fn comment_list_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = list_with(ProjectionArgs {
        fields: Some("creatorx".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(
        io.out_str().is_empty(),
        "nothing should be written on validation error"
    );
}

#[tokio::test]
async fn comment_list_table_fields_is_noop_with_warning() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("id".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(
        io.out_str().contains("Hello world"),
        "table body should still render"
    );
    assert!(
        io.err_str()
            .contains("--fields/--exclude-fields only affect"),
        "table mode should warn: {}",
        io.err_str()
    );
}
