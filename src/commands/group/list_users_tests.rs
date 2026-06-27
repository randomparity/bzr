#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{GroupAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_one_member(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {"id": 10, "name": "alice@test.com", "real_name": "Alice",
                 "email": "alice@test.com", "can_login": true}
            ]
        })))
        .mount(mock)
        .await;
}

fn list_with(details: bool, projection: ProjectionArgs) -> GroupAction {
    GroupAction::ListUsers {
        group: "admin".to_string(),
        details,
        projection,
    }
}

#[tokio::test]
async fn group_list_users_returns_members() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {"id": 10, "name": "alice@test.com", "real_name": "Alice", "email": "alice@test.com"},
                {"id": 11, "name": "bob@test.com", "real_name": "Bob", "email": "bob@test.com"}
            ]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group list_users failed: {result:?}");
}

#[tokio::test]
async fn group_list_users_with_details() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {"id": 10, "name": "alice@test.com", "real_name": "Alice", "email": "alice@test.com", "can_login": true}
            ]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: true,
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "group list_users --details failed: {result:?}"
    );
}

#[tokio::test]
async fn group_list_users_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::group::execute(
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
async fn group_list_users_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_member(&mock).await;

    let action = list_with(
        false,
        ProjectionArgs {
            fields: Some("email".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed[0]["email"], "alice@test.com");
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn group_list_users_details_json_fields_still_projects() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_member(&mock).await;

    let action = list_with(
        true,
        ProjectionArgs {
            fields: Some("email".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed[0]["email"], "alice@test.com");
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn group_list_users_ndjson_fields_projects_each_line() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_member(&mock).await;

    let action = list_with(
        false,
        ProjectionArgs {
            fields: Some("email".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
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
    assert_eq!(io.out_str().trim(), r#"{"email":"alice@test.com"}"#);
}

#[tokio::test]
async fn group_list_users_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = list_with(
        false,
        ProjectionArgs {
            fields: Some("emial".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn group_list_users_table_fields_is_noop_with_warning() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_member(&mock).await;

    let action = list_with(
        false,
        ProjectionArgs {
            fields: Some("email".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(io.out_str().contains("alice@test.com"));
    assert!(io
        .err_str()
        .contains("--fields/--exclude-fields only affect"));
}
