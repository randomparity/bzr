#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::encode_path;
use super::super::UserDetailLevel;
use super::super::USER_FIELDS_BASIC;
use crate::client::test_helpers::{test_client, test_client_hybrid};
use crate::error::BzrError;
use crate::types::{ApiMode, AuthMethod, CreateGroupParams, UpdateGroupParams};

#[tokio::test]
async fn get_group_members_returns_users() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param("group", "admin"))
        .and(query_param("include_fields", USER_FIELDS_BASIC))
        .and(query_param("match", "*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "id": 1,
                    "name": "alice@example.com",
                    "real_name": "Alice",
                    "email": "alice@example.com"
                },
                {
                    "id": 2,
                    "name": "bob@example.com",
                    "real_name": "Bob",
                    "email": "bob@example.com"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let users = client
        .get_group_members("admin", UserDetailLevel::Basic)
        .await
        .unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "alice@example.com");
}

#[tokio::test]
async fn get_group_members_details_sends_include_fields() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param("group", "admin"))
        .and(query_param(
            "include_fields",
            super::super::USER_FIELDS_DETAILED,
        ))
        .and(query_param("match", "*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "id": 1,
                    "name": "alice@example.com",
                    "real_name": "Alice",
                    "email": "alice@example.com",
                    "can_login": true,
                    "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let users = client
        .get_group_members("admin", UserDetailLevel::Detailed)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "alice@example.com");
    assert_eq!(users[0].groups.len(), 1);
    assert_eq!(users[0].groups[0].name, "admin");
    assert_eq!(users[0].can_login, Some(true));
}

#[tokio::test]
async fn get_group_members_empty() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param("group", "nobody"))
        .and(query_param("match", "*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let users = client
        .get_group_members("nobody", UserDetailLevel::Basic)
        .await
        .unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn get_group_members_api_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param("group", "nonexistent"))
        .and(query_param("match", "*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "There is no group named 'nonexistent'."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .get_group_members("nonexistent", UserDetailLevel::Basic)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent"),
        "Expected error to mention group name, got: {msg}"
    );
}

#[tokio::test]
async fn add_user_to_group_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path(format!(
            "/rest/user/{}",
            encode_path("alice@example.com")
        )))
        .and(body_json(
            serde_json::json!({"groups": {"add": ["testers"]}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    client
        .add_user_to_group("alice@example.com", "testers")
        .await
        .unwrap();
}

#[tokio::test]
async fn remove_user_from_group_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path(format!(
            "/rest/user/{}",
            encode_path("bob@example.com")
        )))
        .and(body_json(
            serde_json::json!({"groups": {"remove": ["testers"]}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 2, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    client
        .remove_user_from_group("bob@example.com", "testers")
        .await
        .unwrap();
}

#[tokio::test]
async fn get_group_returns_info() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .and(query_param("membership", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Administrators",
                "is_active": true,
                "membership": []
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let info = client.get_group("admin").await.unwrap();
    assert_eq!(info.name, "admin");
    assert!(info.is_active);
}

#[tokio::test]
async fn get_group_rest_empty_response_returns_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "missing"))
        .and(query_param("membership", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": []
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_group("missing").await.unwrap_err();
    assert!(matches!(
        err,
        BzrError::NotFound {
            resource: "group",
            ..
        }
    ));
}

#[tokio::test]
async fn get_group_forbidden() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_group("secret").await.unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}

#[tokio::test]
async fn hybrid_get_group_32610_falls_back_to_xmlrpc() {
    let mock = MockServer::start().await;

    // REST returns error 32610 (Bugzilla 5.3+ blocks GET for Group.get)
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 32610,
            "message": "For security reasons, you must use HTTP POST to call the 'get' method."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // XML-RPC fallback succeeds
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_group_response(
                1,
                "admin",
                "Administrators",
            )),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let info = client.get_group("admin").await.unwrap();
    assert_eq!(info.name, "admin");
    assert_eq!(info.description, "Administrators");
}

#[tokio::test]
async fn hybrid_get_group_transport_failure_falls_back_to_xmlrpc() {
    // A 5xx response from REST is a transport failure under
    // `is_transport_failure()`. In Hybrid mode it must trigger the
    // XML-RPC retry, not propagate as the final error.
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_group_response(
                1,
                "admin",
                "Administrators",
            )),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let info = client.get_group("admin").await.unwrap();
    assert_eq!(info.name, "admin");
}

#[tokio::test]
async fn rest_get_group_32610_falls_back_to_xmlrpc() {
    let mock = MockServer::start().await;

    // REST returns error 32610 (Bugzilla 5.3+ blocks GET for Group.get)
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 32610,
            "message": "For security reasons, you must use HTTP POST to call the 'get' method."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // XML-RPC fallback succeeds
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_group_response(
                1,
                "admin",
                "Administrators",
            )),
        )
        .expect(1)
        .mount(&mock)
        .await;

    // Uses test_client (Rest mode), not hybrid
    let client = test_client(&mock.uri());
    let info = client.get_group("admin").await.unwrap();
    assert_eq!(info.name, "admin");
    assert_eq!(info.description, "Administrators");
}

#[tokio::test]
async fn xmlrpc_mode_get_group_bypasses_rest() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_group_response(
                1,
                "admin",
                "Administrators",
            )),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = super::BugzillaClient::new(crate::client::BugzillaClientConfig {
        base_url: &mock.uri(),
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::XmlRpc,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();

    let info = client.get_group("admin").await.unwrap();
    assert_eq!(info.name, "admin");
}

#[tokio::test]
async fn hybrid_get_group_api_error_does_not_fall_back() {
    let mock = MockServer::start().await;

    // REST returns a non-retriable API error (not 32610, not transport)
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // XML-RPC should not be called
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let err = client.get_group("secret").await.unwrap_err();
    assert!(
        err.to_string().contains("not authorized"),
        "expected auth error, got: {err}"
    );
}

/// Build a mock XML-RPC Group.get response containing one group.
fn xmlrpc_group_response(id: i64, name: &str, description: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse><params><param><value><struct>
          <member><name>groups</name><value><array><data>
            <value><struct>
              <member><name>id</name><value><int>{id}</int></value></member>
              <member><name>name</name><value><string>{name}</string></value></member>
              <member><name>description</name><value><string>{description}</string></value></member>
              <member><name>is_active</name><value><boolean>1</boolean></value></member>
              <member><name>membership</name><value><array><data></data></array></value></member>
            </struct></value>
          </data></array></value></member>
        </struct></value></param></params></methodResponse>"#
    )
}

#[tokio::test]
async fn create_group_returns_id() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .and(body_json(serde_json::json!({
            "name": "testers",
            "description": "Test team",
            "is_active": true,
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let id = client
        .create_group(&CreateGroupParams {
            name: "testers".into(),
            description: "Test team".into(),
            is_active: true,
        })
        .await
        .unwrap();
    assert_eq!(id, 5);
}

#[tokio::test]
async fn create_group_forbidden() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .create_group(&CreateGroupParams {
            name: "x".into(),
            description: "x".into(),
            is_active: true,
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}

#[tokio::test]
async fn update_group_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/group/testers"))
        .and(body_json(serde_json::json!({
            "description": "Updated testers",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{"id": 5, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = UpdateGroupParams {
        description: Some("Updated testers".into()),
        ..Default::default()
    };
    client.update_group("testers", &params).await.unwrap();
}

#[tokio::test]
async fn update_group_forbidden() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/group/testers"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = UpdateGroupParams::default();
    let err = client.update_group("testers", &params).await.unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}
