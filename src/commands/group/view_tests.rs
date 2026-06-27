#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{GroupAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_one_group(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Admin group",
                "is_active": true,
                "membership": []
            }]
        })))
        .mount(mock)
        .await;
}

fn view_with(projection: ProjectionArgs) -> GroupAction {
    GroupAction::View {
        group: "admin".to_string(),
        projection,
    }
}

#[tokio::test]
async fn group_view_returns_info() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Admin group",
                "is_active": true,
                "membership": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
        projection: ProjectionArgs::default(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "group_view failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["name"], "admin");
    assert_eq!(parsed["description"], "Admin group");
}

#[tokio::test]
async fn group_view_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::group::execute(
        &action,
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

#[tokio::test]
async fn group_view_malformed_json_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn group_view_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_group(&mock).await;

    let action = view_with(ProjectionArgs {
        fields: Some("name".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["name"], "admin");
    assert_eq!(parsed.as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn group_view_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = view_with(ProjectionArgs {
        fields: Some("nam".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(io.out_str().is_empty());
}
