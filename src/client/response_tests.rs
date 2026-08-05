#![expect(clippy::disallowed_methods, clippy::unwrap_used)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::client::test_helpers::test_client;
use crate::client::UserDetailLevel;

fn debug_logging_guard() -> tracing::dispatcher::DefaultGuard {
    let (_capture, guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    guard
}

fn multibyte_body_crossing_preview_boundary() -> String {
    let mut body = "a".repeat(super::BODY_PREVIEW_MAX_BYTES - 1);
    body.push('é');
    body.push_str(" trailing");
    body
}

fn spawn_truncated_http_error_server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _addr)) = listener.accept() else {
            return;
        };
        let _ = stream.read(&mut [0_u8; 1024]);
        let _ = stream
            .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 32\r\n\r\noops");
    });

    (format!("http://127.0.0.1:{port}"), handle)
}

fn assert_log_redacted(log: &str, secret: &str) {
    assert!(
        !log.contains(secret),
        "API key leaked in tracing output: {log}"
    );
    assert!(
        log.contains("Bugzilla_api_key=[REDACTED]"),
        "redaction marker missing from tracing output: {log}"
    );
}

#[test]
fn trace_response_body_redacts_api_key() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::TRACE);
    let secret = "TraceSecret123";
    let body = format!(r#"{{"echo":"Bugzilla_api_key={secret}"}}"#);

    BugzillaClient::parse_body_to_value(&body, "http://bugzilla.test").unwrap();

    assert_log_redacted(&capture.output(), secret);
}

#[test]
fn invalid_json_debug_preview_redacts_api_key() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let secret = "InvalidJsonSecret123";
    let body = format!("not json Bugzilla_api_key={secret}");

    let _ = BugzillaClient::parse_body_to_value(&body, "http://bugzilla.test");

    assert_log_redacted(&capture.output(), secret);
}

#[test]
fn error_payload_debug_message_redacts_api_key() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let secret = "PayloadSecret123";
    let value = serde_json::json!({
        "error": true,
        "code": 102,
        "message": format!("request Bugzilla_api_key={secret} rejected")
    });

    let _ = BugzillaClient::check_bugzilla_200_error(&value, "http://bugzilla.test");

    assert_log_redacted(&capture.output(), secret);
}

#[tokio::test]
async fn http_error_debug_body_redacts_api_key() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let mock = MockServer::start().await;
    let secret = "HttpErrorSecret123";
    Mock::given(method("GET"))
        .and(path("/error"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string(format!("request Bugzilla_api_key={secret} rejected")),
        )
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let response = reqwest::Client::new()
        .get(format!("{}/error", mock.uri()))
        .send()
        .await
        .unwrap();

    let _ = client.check_response_status(response).await;

    assert_log_redacted(&capture.output(), secret);
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
    assert_eq!(bug.summary.as_deref(), Some("test bug"));
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
    let err = client
        .search_users("anyone", UserDetailLevel::Basic)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("500") || msg.contains("Internal Server Error"),
        "expected 500 error: {msg}"
    );
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
fn parse_body_to_value_handles_multibyte_debug_preview_boundary() {
    let _guard = debug_logging_guard();
    let body = multibyte_body_crossing_preview_boundary();

    let err = BugzillaClient::parse_body_to_value(&body, "https://bugzilla.example/rest/bug")
        .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::Deserialize(_)),
        "invalid JSON should return a deserialize error, got: {err}"
    );
}

#[tokio::test]
async fn http_error_preview_handles_multibyte_debug_preview_boundary() {
    let _guard = debug_logging_guard();
    let mock = MockServer::start().await;
    let body = multibyte_body_crossing_preview_boundary();
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500).set_body_string(body.clone()))
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

    let expected = format!("{}…", "a".repeat(super::BODY_PREVIEW_MAX_BYTES - 1));
    assert!(
        matches!(
            &err,
            crate::error::BzrError::HttpStatus { status: 500, body: returned }
                if returned == &expected
        ),
        "expected HTTP 500 with a UTF-8-safe bounded body, got: {err}"
    );
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.error_type(), "http");
}

#[tokio::test]
async fn http_error_preview_redacts_bare_key_crossing_boundary() {
    let secret = "configured-secret";
    let _redaction_guard = crate::bugzilla_auth::active_api_key_test_guard(Some(secret));
    let mock = MockServer::start().await;
    let body = format!(
        "{}{secret} trailing",
        "a".repeat(BODY_PREVIEW_MAX_BYTES - 4)
    );
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500).set_body_string(body))
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

    match &err {
        crate::error::BzrError::HttpStatus { body, .. } => {
            assert!(!body.contains("conf"), "stored key prefix leaked: {body}");
            assert!(
                body.ends_with("[REDACTED] …"),
                "redaction marker missing: {body}"
            );
            assert!(body.len() <= BODY_PREVIEW_MAX_BYTES + '…'.len_utf8());
        }
        other => assert!(matches!(other, crate::error::BzrError::HttpStatus { .. })),
    }
    assert!(
        !err.to_string().contains("conf"),
        "displayed key prefix leaked: {err}"
    );
}

#[tokio::test]
async fn http_error_preview_keeps_marker_after_marked_key_redaction() {
    for marker in ["Bugzilla_api_key=", "Bugzilla_api_key%3D"] {
        let mock = MockServer::start().await;
        let body = format!("{}{marker}{}", "a".repeat(480), "s".repeat(100));
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(500).set_body_string(body))
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

        match &err {
            crate::error::BzrError::HttpStatus { body, .. } => {
                assert!(!body.contains("ssss"), "stored marked key leaked: {body}");
                assert!(
                    body.ends_with("[REDACTED] …"),
                    "stored truncation marker missing: {body}"
                );
            }
            other => assert!(matches!(other, crate::error::BzrError::HttpStatus { .. })),
        }
        let displayed = err.to_string();
        assert!(
            !displayed.contains("ssss"),
            "displayed marked key leaked: {displayed}"
        );
        assert!(
            displayed.ends_with("[REDACTED] …"),
            "displayed truncation marker missing: {displayed}"
        );
    }
}

#[tokio::test]
async fn http_error_body_read_failure_preserves_bounded_context() {
    let (url, handle) = spawn_truncated_http_error_server();
    let client = test_client(&url);
    let resp = client.http.get(&url).send().await.unwrap();

    let err = client.check_response_status(resp).await.unwrap_err();
    handle.join().unwrap();

    match &err {
        crate::error::BzrError::HttpStatus { status, body } => {
            assert_eq!(*status, 500);
            assert!(
                body.contains("failed to read response body"),
                "missing context: {body}"
            );
            assert!(body.len() <= BODY_PREVIEW_MAX_BYTES + '…'.len_utf8());
        }
        other => assert!(matches!(other, crate::error::BzrError::HttpStatus { .. })),
    }
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

#[tokio::test]
async fn api_200_error_with_out_of_range_unsigned_code_is_malformed() {
    let mock = MockServer::start().await;
    let code = u64::try_from(i64::MAX).unwrap() + 1;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": code,
            "message": "code is outside signed range"
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .get_json_query::<serde_json::Value>("group", &[])
        .await
        .unwrap_err();

    assert!(
        matches!(
            &err,
            crate::error::BzrError::Deserialize(message)
                if message.contains("Bugzilla error response")
        ),
        "expected malformed Bugzilla error response, got: {err}"
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
    let result = BugzillaClient::try_envelopes(
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
    let result = BugzillaClient::try_envelopes(
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
    let err = BugzillaClient::try_envelopes(
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
    let result = BugzillaClient::try_envelopes(
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
async fn parse_json_invalid_json_handles_multibyte_debug_preview_boundary() {
    let _guard = debug_logging_guard();
    let mock = MockServer::start().await;
    let body = multibyte_body_crossing_preview_boundary();
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_attachments(42).await.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::Deserialize(_)),
        "invalid JSON should return a deserialize error, got: {err}"
    );

    let msg = err.to_string();
    assert!(
        msg.contains("body preview"),
        "error should include body preview: {msg}"
    );
    assert!(msg.contains('…'), "preview should be truncated: {msg}");
}

/// When multiple present-keyed extractors both fail, only the FIRST error
/// must be surfaced.  The guard `first_error.is_none()` ensures this.
/// Replacing the guard with `true` would let later errors overwrite the first;
/// replacing with `false` would drop all errors — both mutations break this test.
#[test]
fn try_envelopes_preserves_first_error_when_multiple_keyed_extractors_fail() {
    // Both "bugs" and "attachments" keys are present in the JSON, so both
    // extractors run in the first pass.  Each returns a distinct error message.
    let value = serde_json::json!({
        "bugs": "wrong_shape",
        "attachments": "also_wrong"
    });

    let extract_bugs: fn(&serde_json::Value) -> Result<i32> = |_v| {
        Err(crate::error::BzrError::Deserialize(
            "FIRST_ERROR_FROM_BUGS".into(),
        ))
    };
    let extract_attachments: fn(&serde_json::Value) -> Result<i32> = |_v| {
        Err(crate::error::BzrError::Deserialize(
            "SECOND_ERROR_FROM_ATTACHMENTS".into(),
        ))
    };

    let err = BugzillaClient::try_envelopes(
        &value,
        &[("bugs", extract_bugs), ("attachments", extract_attachments)],
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("FIRST_ERROR_FROM_BUGS"),
        "first extractor's error must be preserved: {msg}"
    );
    assert!(
        !msg.contains("SECOND_ERROR_FROM_ATTACHMENTS"),
        "second extractor's error must not replace the first: {msg}"
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

// ── #504: an error payload beside an *empty* data key is not "data" ──────
//
// `has_data_fields` used to test key presence alone, so a server that
// answered a restricted-bug lookup with an error *and* an empty `bugs`
// placeholder had its error downgraded to a warning. The empty list then
// became `NotFound` — "bug not found" for a bug the caller could see.
// ADR 0015: the error is the only thing the server said, so it is fatal.

/// Build a 200 response carrying a Bugzilla error alongside `bugs: <value>`.
fn restricted_bug_body(bugs: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "error": true,
        "code": 102,
        "message": "You are not authorized to access bug #216593.",
        "bugs": bugs,
    })
}

async fn assert_restricted_bug_is_fatal(bugs: &serde_json::Value, case: &str) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/216593"))
        .respond_with(ResponseTemplate::new(200).set_body_json(restricted_bug_body(bugs)))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_bug("216593", None, None).await.unwrap_err();

    assert!(
        matches!(err, BzrError::Api { code: 102, .. }),
        "{case}: expected Api{{102}}, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("not authorized"),
        "{case}: server message must be relayed: {msg}"
    );
    assert!(
        !msg.contains("not found"),
        "{case}: must not be masked as not-found: {msg}"
    );
}

#[tokio::test]
async fn error_with_empty_array_data_key_is_fatal() {
    assert_restricted_bug_is_fatal(&serde_json::json!([]), "empty array").await;
}

#[tokio::test]
async fn error_with_empty_object_data_key_is_fatal() {
    assert_restricted_bug_is_fatal(&serde_json::json!({}), "empty object").await;
}

#[tokio::test]
async fn error_with_null_data_key_is_fatal() {
    assert_restricted_bug_is_fatal(&serde_json::Value::Null, "null").await;
}

#[tokio::test]
async fn error_beside_populated_data_key_stays_lenient() {
    // Regression guard for the IBM LTC accommodation: an extension's error
    // alongside real data is informational — the data is the answer.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/216593"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(restricted_bug_body(
                &serde_json::json!([{"id": 216_593, "summary": "restricted", "status": "NEW"}]),
            )),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client.get_bug("216593", None, None).await.unwrap();
    assert_eq!(bug.id, 216_593);
}

#[tokio::test]
async fn error_beside_nonempty_map_data_key_stays_lenient() {
    // `bug/<id>/comment` answers with a *map* keyed by bug id. A bug with no
    // comments still yields `bugs: {"42": {"comments": []}}` — the outer map
    // has a key, so it is data and the accompanying error stays a warning.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 100_500,
            "message": "MirrorTool internal error",
            "bugs": {"42": {"comments": []}},
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    assert!(client
        .get_comments_since(42, None)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn rejected_mutation_with_empty_data_key_is_not_success() {
    // `put_json`'s body check exists because some deployments report a
    // rejected mutation with a 200 status. An error payload carrying an empty
    // `bugs: []` defeated that guard the same way it defeated `bug view`:
    // `has_data_fields` saw the key and downgraded the error, so a failed
    // update was reported as success (#504, ADR 0015).
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 115,
            "message": "You are not permitted to edit bugs in product Secret.",
            "bugs": [],
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .update_bug(42, &crate::types::bug::UpdateBugParams::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Api { code: 115, .. }),
        "a rejected mutation must not be reported as success, got {err:?}"
    );
}

/// The two `BzrError::Api` construction sites both build their message from
/// server-supplied text. Whichever one fires, the rendered error must be
/// redacted — the assertion is on `to_string()`, the shared `Display` seam,
/// not on a redaction repeated at each site.
#[tokio::test]
async fn api_error_from_4xx_body_redacts_echoed_api_key() {
    let _redaction_guard = crate::bugzilla_auth::active_api_key_test_guard(Some("test-key"));
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": true,
            "code": 32000,
            "message": "invalid request for test-key"
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
    let msg = err.to_string();
    assert!(
        matches!(&err, crate::error::BzrError::Api { code: 32000, .. }),
        "expected Api error from the 4xx body, got: {msg}"
    );
    assert!(!msg.contains("test-key"), "key leaked: {msg}");
    assert!(msg.contains("invalid request for [REDACTED]"), "{msg}");
}

#[tokio::test]
async fn api_error_from_200_error_payload_redacts_echoed_api_key() {
    let _redaction_guard = crate::bugzilla_auth::active_api_key_test_guard(Some("test-key"));
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 301,
            "message": "denied for test-key"
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_product("Secret").await.unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("test-key"), "key leaked: {msg}");
    assert!(msg.contains("denied for [REDACTED]"), "{msg}");
    assert!(msg.contains("301"), "code must survive: {msg}");
}

/// A non-JSON 4xx/5xx body falls through to `HttpStatus`, which carries the
/// raw body — the same gap, so it gets the same seam. The body here is the
/// realistic shape: an untruncated, multi-line error page echoing the request
/// URI twice, which a first-match-only redaction would leave half-exposed.
#[tokio::test]
async fn http_status_from_non_json_body_redacts_echoed_api_key() {
    let _redaction_guard = crate::bugzilla_auth::active_api_key_test_guard(Some("test-key"));
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500).set_body_string(
            "<title>500 invalid test-key</title>\n\
             upstream rejected test-key",
        ))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .search_users("anyone", UserDetailLevel::Basic)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        matches!(&err, crate::error::BzrError::HttpStatus { status: 500, .. }),
        "expected HttpStatus from the non-JSON body, got: {msg}"
    );
    assert!(!msg.contains("test-key"), "key leaked: {msg}");
    assert_eq!(
        msg.matches("[REDACTED]").count(),
        2,
        "both echoes must be redacted: {msg}"
    );
    assert!(
        msg.contains("upstream rejected"),
        "body text must survive: {msg}"
    );
}
