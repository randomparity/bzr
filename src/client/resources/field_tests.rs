#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::encode_path;
use crate::client::test_helpers::test_client;
use crate::error::BzrError;
use crate::types::resolve_field_alias;

#[test]
fn resolve_field_alias_maps_status() {
    assert_eq!(resolve_field_alias("status").as_ref(), "bug_status");
}

#[test]
fn resolve_field_alias_maps_severity() {
    assert_eq!(resolve_field_alias("severity").as_ref(), "bug_severity");
}

#[test]
fn resolve_field_alias_maps_id() {
    assert_eq!(resolve_field_alias("id").as_ref(), "bug_id");
}

#[test]
fn resolve_field_alias_maps_type() {
    assert_eq!(resolve_field_alias("type").as_ref(), "bug_type");
}

#[test]
fn resolve_field_alias_maps_group() {
    assert_eq!(resolve_field_alias("group").as_ref(), "bug_group");
}

#[test]
fn resolve_field_alias_maps_file_loc() {
    assert_eq!(resolve_field_alias("file_loc").as_ref(), "bug_file_loc");
}

#[test]
fn resolve_field_alias_passes_through_unknown() {
    assert_eq!(resolve_field_alias("priority").as_ref(), "priority");
}

#[test]
fn resolve_field_alias_passes_through_already_prefixed() {
    assert_eq!(resolve_field_alias("bug_status").as_ref(), "bug_status");
}

#[test]
fn resolve_field_alias_is_case_insensitive() {
    assert_eq!(resolve_field_alias("Status").as_ref(), "bug_status");
    assert_eq!(resolve_field_alias("SEVERITY").as_ref(), "bug_severity");
}

#[tokio::test]
async fn get_field_values_returns_values() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/field/bug/{}",
            encode_path("bug_status")
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true, "can_change_to": [{"name": "ASSIGNED"}, {"name": "RESOLVED"}]},
                    {"name": "RESOLVED", "sort_key": 500, "is_active": true}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values("status").await.unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].name.as_deref(), Some("NEW"));
    let transitions = values[0].can_change_to.as_ref().unwrap();
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].name, "ASSIGNED");
}

#[tokio::test]
async fn get_field_values_defaults_omitted_values_to_empty() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/field/bug/{}",
            encode_path("bug_status")
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "bug_status", "type": 2}]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values("status").await.unwrap();

    assert!(values.is_empty());
}

#[tokio::test]
async fn get_field_values_encodes_resolved_field_name_as_one_path_segment() {
    let mock = MockServer::start().await;
    let field_name = "cf_release/channel?include_fields=id% raw";
    Mock::given(method("GET"))
        .and(path(
            "/rest/field/bug/cf%5Frelease%2Fchannel%3Finclude%5Ffields%3Did%25%20raw",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "cf_release/channel?include_fields=id% raw", "values": []}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values(field_name).await.unwrap();

    assert!(values.is_empty());
}

#[tokio::test]
async fn get_field_values_rejects_empty_or_dot_segments_without_a_request() {
    for field_name in ["", ".", ".."] {
        let mock = MockServer::start().await;
        let client = test_client(&mock.uri());

        let result = client.get_field_values(field_name).await;

        assert!(matches!(
            result,
            Err(crate::error::BzrError::InputValidation {
                field: Some(ref field),
                value: Some(ref value),
                ..
            }) if field == "field" && value == field_name
        ));
        assert!(mock.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn get_field_values_resolves_severity_alias() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/field/bug/{}",
            encode_path("bug_severity")
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_severity",
                "values": [
                    {"name": "blocker", "sort_key": 100, "is_active": true},
                    {"name": "normal", "sort_key": 200, "is_active": true}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values("severity").await.unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].name.as_deref(), Some("blocker"));
}

#[tokio::test]
async fn get_field_values_unrecognized_field_returns_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/nonexistent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"fields": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_field_values("nonexistent").await.unwrap_err();
    assert!(
        matches!(
            err,
            BzrError::NotFound {
                resource: "field",
                ..
            }
        ),
        "expected NotFound, got: {err}"
    );
}
