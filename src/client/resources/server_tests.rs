#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::{
    encode_path,
    test_helpers::{test_client, test_client_anon},
};
use tracing::instrument::WithSubscriber as _;

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
        .and(path(format!(
            "/rest/field/bug/{}",
            encode_path("bug_status")
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "", "can_change_to": [{"name": "NEW"}]},
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
            "parameters": {"maxattachmentsize": "1000"}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.max_attachment_size, Some(1_024_000));
}

#[tokio::test]
async fn server_capabilities_keeps_numeric_attachment_size_compatibility() {
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
async fn server_capabilities_accepts_string_and_missing_field_types_and_values() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [
                {"name": "cf_string_type", "type": "2", "is_custom": true,
                 "values": [{"name": "1.0"}]},
                {"name": "cf_missing_type", "is_custom": true}
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client.server_capabilities().await.unwrap();

    assert_eq!(caps.custom_fields[0].field_type, "single_select");
    assert_eq!(caps.custom_fields[0].values, vec!["1.0"]);
    assert_eq!(caps.custom_fields[1].field_type, "unknown");
    assert!(caps.custom_fields[1].values.is_empty());
}

#[tokio::test]
async fn server_capabilities_logs_response_shape_attachment_failure() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/parameters"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "parameters": {
                "maxattachmentsize": "not-a-number Bugzilla_api_key=test-key marker"
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client
        .server_capabilities()
        .with_current_subscriber()
        .await
        .unwrap();
    let output = capture.output();

    assert_eq!(caps.max_attachment_size, None);
    assert!(output.contains("reason=response_shape"), "{output}");
    assert!(output.contains("marker"), "{output}");
    assert!(!output.contains("test-key"), "{output}");
}

#[tokio::test]
async fn server_capabilities_nulls_attachment_size_on_parameters_error() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_status_field(&mock).await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/parameters"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string("Unauthorized Bugzilla_api_key=test-key marker"),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let caps = client
        .server_capabilities()
        .with_current_subscriber()
        .await
        .unwrap();
    let output = capture.output();

    assert_eq!(caps.max_attachment_size, None);
    assert!(output.contains("reason=request"), "{output}");
    assert!(output.contains("marker"), "{output}");
    assert!(!output.contains("test-key"), "{output}");
}

#[tokio::test]
async fn server_capabilities_empty_status_field_yields_no_transitions() {
    let mock = MockServer::start().await;
    mount_version(&mock, "5.0.4").await;
    mount_all_fields(&mock).await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/rest/field/bug/{}",
            encode_path("bug_status")
        )))
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
            // TODO(#626): every stock server stringifies /parameters values; #626 owns the fix.
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

// ── Extensions probe transport (ADR-0052, amended 2026-09-06) ──────────
//
// Each case mounts BOTH surfaces with explicit counts, so "the probe went to
// the other transport" and "no probe was issued" both fail rather than passing
// silently. The advertised XML-RPC body is the shape a Red Hat server sends.

const XMLRPC_ADVERTISED: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct>",
    r"<member><name>RedHat</name><value><struct>",
    r"<member><name>version</name><value><string>1.0</string></value></member>",
    r"</struct></value></member>",
    r"</struct></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

/// The body a real Bugzilla returns for an absent REST endpoint — verified
/// against 5.0.6 and 5.3.3+. It classifies as `BzrError::Api`, which
/// `is_transport_failure()` does NOT match; this is the shape that makes the
/// Hybrid fallback unconditional rather than predicate-guarded.
fn bugzilla_error_envelope() -> serde_json::Value {
    serde_json::json!({
        "error": true,
        "code": 32614,
        "message": "A REST API resource was not found for 'GET /extensions'."
    })
}

async fn mount_rest_extensions(mock: &MockServer, response: ResponseTemplate, expect: u64) {
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(response)
        .expect(expect)
        .mount(mock)
        .await;
}

async fn mount_xmlrpc_extensions(mock: &MockServer, response: ResponseTemplate, expect: u64) {
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(response)
        .expect(expect)
        .mount(mock)
        .await;
}

fn rest_advertised() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"extensions": {"RedHat": {"version": "1.0"}}}))
}

fn xmlrpc_advertised() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_string(XMLRPC_ADVERTISED)
}

#[tokio::test]
async fn server_extensions_uses_xmlrpc_under_xmlrpc_mode() {
    let mock = MockServer::start().await;
    mount_rest_extensions(&mock, rest_advertised(), 0).await;
    mount_xmlrpc_extensions(&mock, xmlrpc_advertised(), 1).await;

    let ext = crate::client::test_helpers::test_client_xmlrpc(&mock.uri())
        .server_extensions()
        .await
        .unwrap();

    assert!(ext.extensions.contains_key("RedHat"));
}

#[tokio::test]
async fn server_extensions_uses_rest_under_rest_mode() {
    let mock = MockServer::start().await;
    mount_rest_extensions(&mock, rest_advertised(), 1).await;
    mount_xmlrpc_extensions(&mock, xmlrpc_advertised(), 0).await;

    let ext = test_client(&mock.uri()).server_extensions().await.unwrap();

    assert!(ext.extensions.contains_key("RedHat"));
}

#[tokio::test]
async fn server_extensions_hybrid_prefers_rest() {
    let mock = MockServer::start().await;
    mount_rest_extensions(&mock, rest_advertised(), 1).await;
    mount_xmlrpc_extensions(&mock, xmlrpc_advertised(), 0).await;

    let ext = crate::client::test_helpers::test_client_hybrid(&mock.uri())
        .server_extensions()
        .await
        .unwrap();

    assert!(ext.extensions.contains_key("RedHat"));
}

/// The case that bites. A guard of `Err(e) if e.is_transport_failure()` does
/// NOT fire here, because the envelope classifies as `BzrError::Api` — so this
/// is the test that distinguishes the unconditional fallback from a predicate.
#[tokio::test]
async fn server_extensions_hybrid_falls_back_on_bugzilla_error_envelope() {
    let mock = MockServer::start().await;
    mount_rest_extensions(
        &mock,
        ResponseTemplate::new(404).set_body_json(bugzilla_error_envelope()),
        1,
    )
    .await;
    mount_xmlrpc_extensions(&mock, xmlrpc_advertised(), 1).await;

    let ext = crate::client::test_helpers::test_client_hybrid(&mock.uri())
        .server_extensions()
        .await
        .expect("an enveloped REST failure must fall back to XML-RPC");

    assert!(ext.extensions.contains_key("RedHat"));
}

/// The shape that passes under either rule, kept so the two together *prove*
/// the fallback is unconditional rather than assume it.
#[tokio::test]
async fn server_extensions_hybrid_falls_back_on_bodyless_failure() {
    let mock = MockServer::start().await;
    mount_rest_extensions(&mock, ResponseTemplate::new(503), 1).await;
    mount_xmlrpc_extensions(&mock, xmlrpc_advertised(), 1).await;

    let ext = crate::client::test_helpers::test_client_hybrid(&mock.uri())
        .server_extensions()
        .await
        .expect("a bodyless REST failure must fall back to XML-RPC");

    assert!(ext.extensions.contains_key("RedHat"));
}

/// Both failing must stay an error: an empty list here would render as a
/// settled *absent* instead of *undetermined*, which is the one fail-open this
/// arm could introduce. The message names both attempts, because the `info`
/// warn-level trace of the REST failure is a trace event, and a `--json`
/// consumer reads the error body rather than the trace.
#[tokio::test]
async fn server_extensions_hybrid_both_transports_failing_is_an_error() {
    let mock = MockServer::start().await;
    mount_rest_extensions(
        &mock,
        ResponseTemplate::new(404).set_body_json(bugzilla_error_envelope()),
        1,
    )
    .await;
    mount_xmlrpc_extensions(&mock, ResponseTemplate::new(500), 1).await;

    let err = crate::client::test_helpers::test_client_hybrid(&mock.uri())
        .server_extensions()
        .await
        .expect_err("both transports failing must not yield an empty list");

    let message = err.to_string();
    assert!(message.contains("REST"), "{message}");
    assert!(message.contains("XML-RPC"), "{message}");
}
