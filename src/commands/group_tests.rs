#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::GroupAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

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
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "group_view failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["name"], "admin");
    assert_eq!(parsed["description"], "Admin group");
}

#[tokio::test]
async fn group_create_sends_post() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::Create {
        name: "new-group".into(),
        description: "A test group".into(),
        is_active: true,
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "group create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 5);
}

#[tokio::test]
async fn group_update_sends_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/admin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"groups": [{"changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::Update {
        group: "admin".into(),
        description: Some("Updated description".into()),
        is_active: None,
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok(), "group update failed: {result:?}");
}

#[tokio::test]
async fn group_add_user_sends_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // add_user_to_group sends PUT /rest/user/{user} with group membership body
    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"users": [{"id": 1, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::AddUser {
        group: "admin".into(),
        user: "alice@test.com".into(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok(), "group add_user failed: {result:?}");
}

#[tokio::test]
async fn group_remove_user_sends_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/bob%40test%2Ecom"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"users": [{"id": 2, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::RemoveUser {
        group: "admin".into(),
        user: "bob@test.com".into(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok(), "group remove_user failed: {result:?}");
}

#[tokio::test]
async fn group_list_users_returns_members() {
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
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok(), "group list_users failed: {result:?}");
}

#[tokio::test]
async fn group_list_users_with_details() {
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
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(
        result.is_ok(),
        "group list_users --details failed: {result:?}"
    );
}

#[tokio::test]
async fn group_view_http_500_returns_error() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn group_view_malformed_json_returns_error() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
}
