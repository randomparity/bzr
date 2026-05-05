#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use test_helpers::{test_client, test_client_query_param};

#[test]
fn safe_url_strips_query_params() {
    let url = reqwest::Url::parse(&format!(
        "https://bugzilla.example.com/rest/bug/1?{}=secret",
        crate::http::AUTH_QUERY_PARAM
    ))
    .unwrap();
    let safe = BugzillaClient::safe_url(&url);
    assert!(
        !safe.contains("secret"),
        "API key should be stripped: {safe}"
    );
    assert!(
        safe.contains("/rest/bug/1"),
        "path should be preserved: {safe}"
    );
}

#[test]
fn safe_url_preserves_path() {
    let url = reqwest::Url::parse("https://bugzilla.example.com/rest/bug/42").unwrap();
    let safe = BugzillaClient::safe_url(&url);
    assert_eq!(safe, "https://bugzilla.example.com/rest/bug/42");
}

#[test]
fn new_trims_trailing_slash_and_keeps_email_hint() {
    let client = BugzillaClient::new(
        "https://bugzilla.example.com/",
        "test-key",
        AuthMethod::Header,
        ApiMode::Rest,
        Some("user@example.com"),
        &crate::tls::TlsConfig::default(),
    )
    .unwrap();

    assert_eq!(client.base_url, "https://bugzilla.example.com");
    assert_eq!(client.email_hint.as_deref(), Some("user@example.com"));
}

#[test]
fn apply_auth_adds_query_param_credentials() {
    let client = test_client_query_param("https://bugzilla.example.com");
    let request = client
        .apply_auth(client.http.get(client.url("bug")))
        .build()
        .unwrap();
    let expected_query = format!("{AUTH_QUERY_PARAM}=test-key");
    assert_eq!(request.url().query(), Some(expected_query.as_str()));
}

#[test]
fn alternate_auth_rejects_invalid_header_characters() {
    let client = BugzillaClient::new(
        "https://bugzilla.example.com",
        "bad\nkey",
        AuthMethod::QueryParam,
        ApiMode::Rest,
        None,
        &crate::tls::TlsConfig::default(),
    )
    .unwrap();

    let builder = client.http.get(client.url("bug"));
    let err = client.apply_alternate_auth(builder).unwrap_err();
    assert!(err.to_string().contains("invalid header characters"));
}

#[tokio::test]
async fn api_error_with_200_status() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 301,
            "message": "You are not authorized to access that product."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_product("Secret").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("301"), "expected error code 301: {msg}");
    assert!(
        msg.contains("not authorized"),
        "expected auth error message: {msg}"
    );
}

#[tokio::test]
async fn api_error_with_200_and_data_returns_data() {
    // Some servers (e.g. IBM LTC) return error fields alongside real
    // data. The data should be used and the error logged as a warning.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 100_500,
            "message": "MirrorTool internal error",
            "bugs": [{"id": 42, "summary": "test bug", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client.get_bug("42", None, None).await.unwrap();
    assert_eq!(bug.id, 42);
    assert_eq!(bug.summary, "test bug");
}

#[tokio::test]
async fn http_500_returns_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.search_users("anyone", false).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("500") || msg.contains("Internal Server Error"),
        "expected 500 error: {msg}"
    );
}

#[tokio::test]
async fn auth_fallback_header_to_query_param_on_401() {
    let mock = MockServer::start().await;
    // Success response requires query param auth (registered first)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "name": "alice@example.com"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    // First request returns 401 (registered second, checked first by LIFO)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let users = client.search_users("alice", false).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "alice@example.com");
}

#[tokio::test]
async fn auth_fallback_query_param_to_header_on_401() {
    let mock = MockServer::start().await;
    // Success response requires header auth (registered first)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(wiremock::matchers::header(
            crate::http::AUTH_HEADER_NAME,
            "test-key",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 2, "name": "bob@example.com"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    // First request returns 401 (registered second, checked first by LIFO)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_query_param(&mock.uri());
    let users = client.search_users("bob", false).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "bob@example.com");
}

#[tokio::test]
async fn auth_fallback_both_fail_returns_original_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.search_users("anyone", false).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("410") || msg.contains("log in"),
        "expected auth error: {msg}"
    );
}

#[tokio::test]
async fn non_401_errors_do_not_trigger_fallback() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.search_users("anyone", false).await.unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}

#[tokio::test]
async fn api_error_with_string_code_parsed_correctly() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": true,
            "code": "32610",
            "message": "For security reasons, you must use HTTP POST."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let resp = client
        .http
        .get(format!("{}/rest/group", mock.uri()))
        .send()
        .await
        .unwrap();
    let err = client.check_response_status(resp).await.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::Api { code: 32610, .. }),
        "expected Api error with code 32610, got: {err}"
    );
}

#[test]
fn error_response_parses_unsigned_integer_code() {
    let json = r#"{"error":true,"code":32610,"message":"x"}"#;
    let err: super::ErrorResponse = serde_json::from_str(json).unwrap();
    assert_eq!(err.code, 32610);
}

#[test]
fn error_response_parses_negative_integer_code() {
    let json = r#"{"error":true,"code":-7,"message":"x"}"#;
    let err: super::ErrorResponse = serde_json::from_str(json).unwrap();
    assert_eq!(err.code, -7);
}

#[tokio::test]
async fn api_200_error_without_code_field_uses_minus_one() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "message": "no code"
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .get_json_query::<serde_json::Value>("group", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::Api { code: -1, .. }),
        "expected Api error with code -1, got: {err}"
    );
}

#[tokio::test]
async fn api_200_error_with_string_code_parsed_correctly() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": "32610",
            "message": "For security reasons, you must use HTTP POST."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err: crate::error::BzrError = client
        .get_json_query::<serde_json::Value>("group", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::Api { code: 32610, .. }),
        "expected Api error with code 32610, got: {err}"
    );
}

#[test]
fn format_body_preview_returns_short_body_unchanged_in_content() {
    let body = r#"{"error":false,"attachments":[]}"#;
    let preview = super::format_body_preview(body);
    assert!(
        preview.contains(r#""attachments":[]"#),
        "should contain original JSON: {preview}"
    );
    assert!(
        !preview.ends_with('…'),
        "short body should not be truncated: {preview}"
    );
}

#[test]
fn format_body_preview_truncates_long_body_with_ellipsis() {
    let body = "x".repeat(2048);
    let preview = super::format_body_preview(&body);
    assert!(
        preview.ends_with('…'),
        "long body should end with ellipsis: ...{}",
        &preview[preview.len().saturating_sub(20)..]
    );
    // Length check: 512 'x' chars + 1 ellipsis char (3 bytes UTF-8) = 515 bytes max for the content.
    assert!(
        preview.chars().count() <= 513,
        "preview should be <=513 chars (512 + ellipsis), got {}",
        preview.chars().count()
    );
}

#[test]
fn format_body_preview_redacts_api_key_in_body() {
    let body = r#"{"echo":"http://h/rest/bug?Bugzilla_api_key=Sup3rSecret&x=1"}"#;
    let preview = super::format_body_preview(body);
    assert!(
        !preview.contains("Sup3rSecret"),
        "API key must be redacted: {preview}"
    );
    assert!(
        preview.contains("Bugzilla_api_key=[REDACTED]"),
        "redaction marker missing: {preview}"
    );
}

#[test]
fn format_body_preview_collapses_internal_whitespace() {
    let body = "{\n  \"a\": 1,\n\t\"b\": 2\n}";
    let preview = super::format_body_preview(body);
    assert!(
        !preview.contains('\n'),
        "newlines should be collapsed: {preview:?}"
    );
    assert!(
        !preview.contains('\t'),
        "tabs should be collapsed: {preview:?}"
    );
}

#[test]
fn format_body_preview_truncates_on_utf8_boundary() {
    // 200 ASCII chars + 200 multi-byte chars (3 bytes each, ☃ = U+2603) = 800 bytes total.
    // Truncation at 512 *bytes* must not split a multi-byte character.
    let mut body = "a".repeat(200);
    for _ in 0..200 {
        body.push('☃');
    }
    let preview = super::format_body_preview(&body);
    // If we sliced mid-codepoint, this would panic before reaching the assert.
    // Confirm the trailing ellipsis is intact (proves no panic and proves truncation occurred).
    assert!(preview.ends_with('…'), "expected truncation: {preview}");
}

#[test]
fn format_body_preview_handles_empty_body() {
    let preview = super::format_body_preview("");
    assert_eq!(preview, "", "empty body should produce empty preview");
}

#[test]
fn try_envelopes_returns_first_candidate_match() {
    let value = serde_json::json!({"bugs": {"42": [{"id": 1}]}});
    let extract_bugs: fn(&serde_json::Value) -> Result<i32> = |_v| Ok(1);
    let extract_attachments: fn(&serde_json::Value) -> Result<i32> = |_v| Ok(2);
    let result = super::BugzillaClient::try_envelopes(
        &value,
        &[("bugs", extract_bugs), ("attachments", extract_attachments)],
    )
    .unwrap();
    assert_eq!(
        result, 1,
        "should pick `bugs` extractor when `bugs` key is present"
    );
}

#[test]
fn try_envelopes_falls_back_to_alt_envelope() {
    let value = serde_json::json!({"attachments": [{"id": 1}]});
    let extract_bugs: fn(&serde_json::Value) -> Result<i32> = |_v| Ok(1);
    let extract_attachments: fn(&serde_json::Value) -> Result<i32> = |_v| Ok(2);
    let result = super::BugzillaClient::try_envelopes(
        &value,
        &[("bugs", extract_bugs), ("attachments", extract_attachments)],
    )
    .unwrap();
    assert_eq!(
        result, 2,
        "should pick `attachments` extractor when only `attachments` key is present"
    );
}

#[test]
fn try_envelopes_returns_first_error_when_no_candidate_matches() {
    let value = serde_json::json!({"unknown_key": "unknown_value"});
    let extract_bugs: fn(&serde_json::Value) -> Result<i32> = |_v| {
        Err(crate::error::BzrError::Deserialize(
            "bugs extractor failed".into(),
        ))
    };
    let extract_attachments: fn(&serde_json::Value) -> Result<i32> = |_v| {
        Err(crate::error::BzrError::Deserialize(
            "attachments extractor failed".into(),
        ))
    };
    let err = super::BugzillaClient::try_envelopes(
        &value,
        &[("bugs", extract_bugs), ("attachments", extract_attachments)],
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("tried envelopes"),
        "should mention attempted envelopes: {msg}"
    );
    assert!(msg.contains("bugs"), "should list 'bugs': {msg}");
    assert!(
        msg.contains("attachments"),
        "should list 'attachments': {msg}"
    );
    assert!(
        msg.contains("bugs extractor failed"),
        "should preserve first extractor's error: {msg}"
    );
    assert!(
        msg.contains("body preview"),
        "should include body preview: {msg}"
    );
    assert!(
        msg.contains("unknown_key"),
        "preview should contain Value contents: {msg}"
    );
}

#[test]
fn try_envelopes_falls_through_when_keyed_extractor_errors() {
    // The `bugs` key is present but its value can't be extracted (wrong shape).
    // The fallback `attachments` extractor (no key required) should still run.
    let value = serde_json::json!({"bugs": "not_an_object", "attachments": [{"id": 1}]});
    let extract_bugs: fn(&serde_json::Value) -> Result<i32> =
        |_v| Err(crate::error::BzrError::Deserialize("bad bugs shape".into()));
    let extract_attachments: fn(&serde_json::Value) -> Result<i32> = |_v| Ok(2);
    let result = super::BugzillaClient::try_envelopes(
        &value,
        &[("bugs", extract_bugs), ("attachments", extract_attachments)],
    )
    .unwrap();
    assert_eq!(result, 2);
}

#[test]
fn format_body_preview_handles_exactly_512_byte_body() {
    // A body whose length exactly equals the truncation threshold should
    // be returned in full with no ellipsis (off-by-one boundary check).
    let body = "a".repeat(512);
    let preview = super::format_body_preview(&body);
    assert_eq!(
        preview.chars().count(),
        512,
        "exact-512 body should not be truncated"
    );
    assert!(
        !preview.ends_with('…'),
        "exact-512 body should have no ellipsis: ...{}",
        &preview[preview.len().saturating_sub(20)..]
    );
}

#[tokio::test]
async fn parse_json_includes_body_preview_on_typed_failure() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            // Wrong shape — has neither `bugs` nor matches AttachmentBugResponse.
            "wrong_key": [1, 2, 3]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_attachments(42).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("body preview"),
        "error should include body preview: {msg}"
    );
    assert!(
        msg.contains("wrong_key"),
        "preview should contain offending JSON keys: {msg}"
    );
}

#[tokio::test]
async fn parse_json_includes_body_preview_on_invalid_json() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not valid json"))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_attachments(42).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("body preview"),
        "error should include body preview: {msg}"
    );
    assert!(
        msg.contains("not valid json"),
        "preview should contain raw body: {msg}"
    );
}

#[tokio::test]
async fn get_json_value_returns_parsed_value_without_typed_check() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/anything"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "arbitrary_key": "arbitrary_value",
            "nested": {"inner": 42}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let value = client.get_json_value("anything").await.unwrap();
    assert_eq!(value["arbitrary_key"], "arbitrary_value");
    assert_eq!(value["nested"]["inner"], 42);
}

#[tokio::test]
async fn get_json_value_runs_check_bugzilla_200_error() {
    // A 200 response with `error: true` and no data fields must still
    // produce a BzrError::Api — get_json_value should run the same
    // 200-error check that get_json does.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/anything"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 301,
            "message": "denied"
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("anything").await.unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::Api { code: 301, .. }),
        "expected Api error, got: {err}"
    );
}

#[tokio::test]
async fn parse_json_redacts_api_key_in_body_preview() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"echo":"http://h/rest?Bugzilla_api_key=LeakedKey9","wrong":true}"#,
        ))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_attachments(42).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        !msg.contains("LeakedKey9"),
        "API key must not appear in error: {msg}"
    );
    assert!(
        msg.contains("[REDACTED]"),
        "redaction marker should be present: {msg}"
    );
}
