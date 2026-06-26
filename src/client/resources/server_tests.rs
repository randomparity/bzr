#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::{test_client, test_client_anon};

/// Mock `/rest/version` returning the given version string.
async fn mount_version(mock: &MockServer, version: &str) {
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "version": version })),
        )
        .mount(mock)
        .await;
}

/// Mock `/rest/field/bug/bug_status` (the resolved alias for `status`) with two
/// statuses, one carrying transitions and one without.
async fn mount_status_field(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "values": [
                    {"name": "NEW", "can_change_to": [{"name": "ASSIGNED"}, {"name": "RESOLVED"}]},
                    {"name": "RESOLVED"}
                ]
            }]
        })))
        .mount(mock)
        .await;
}

/// Mock `/rest/field/bug` (all fields) with one custom field and one built-in.
async fn mount_all_fields(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [
                {"name": "cf_release", "type": 2, "is_custom": true,
                 "values": [{"name": "1.0"}, {"name": "2.0"}]},
                {"name": "priority", "type": 2, "is_custom": false, "values": []}
            ]
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn server_capabilities_assembles_documented_shape() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.version, "5.0.4");
    assert_eq!(caps.api_modes, vec!["rest".to_string()]);
    assert_eq!(caps.auth_modes, vec!["api_key".to_string()]);
    assert!(caps.supports_comments);
    assert!(caps.supports_flag_requests);
    assert!(caps.flag_types.is_none());

    assert_eq!(caps.status_transitions.len(), 1);
    assert_eq!(caps.status_transitions[0].from, "NEW");
    assert_eq!(
        caps.status_transitions[0].can_change_to,
        vec!["ASSIGNED".to_string(), "RESOLVED".to_string()]
    );

    assert_eq!(caps.custom_fields.len(), 1);
    assert_eq!(caps.custom_fields[0].name, "cf_release");
    assert_eq!(caps.custom_fields[0].field_type, "single_select");
    assert_eq!(
        caps.custom_fields[0].values,
        vec!["1.0".to_string(), "2.0".to_string()]
    );
}

#[tokio::test]
async fn server_capabilities_normalizes_attachment_size_to_bytes() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/parameters"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "parameters": {"maxattachmentsize": 1000}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.max_attachment_size, Some(1_024_000));
}

#[tokio::test]
async fn server_capabilities_nulls_attachment_size_on_parameters_error() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/parameters"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.max_attachment_size, None);
}

#[tokio::test]
async fn server_capabilities_empty_status_field_yields_no_transitions() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"fields": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert!(caps.status_transitions.is_empty());
}

#[tokio::test]
async fn server_capabilities_credentialless_skips_parameters_fetch() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;
    // If the credentialless path ever issues this request, expect(0) fails the test.
    Mock::given(method("GET"))
        .and(path("/rest/parameters"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "parameters": {"maxattachmentsize": 1000}
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_anon(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.max_attachment_size, None);
}

#[tokio::test]
async fn server_version_returns_version() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let ver = client.server_version().await.unwrap();
    assert_eq!(ver.version, "5.0.4");
}

#[tokio::test]
async fn server_extensions_returns_map() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "extensions": {
                "BmpConvert": {"version": "1.0"},
                "InlineHistory": {"version": "2.1"}
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let ext = client.server_extensions().await.unwrap();
    assert_eq!(ext.extensions.len(), 2);
    assert!(ext.extensions.contains_key("BmpConvert"));
}
