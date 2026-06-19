#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::AuthMethod;

#[test]
fn apply_auth_to_request_adds_header_auth() {
    let client = reqwest::Client::new();
    let header = reqwest::header::HeaderValue::from_static("secret-key");
    let request = apply_auth_to_request(
        client.get("https://bugzilla.example/rest/bug/1"),
        Some(&header),
        None,
    )
    .build()
    .unwrap();

    assert_eq!(request.headers().get(AUTH_HEADER_NAME).unwrap(), &header);
    assert_eq!(request.url().query(), None);
}

#[test]
fn apply_auth_to_request_adds_query_param_auth() {
    let client = reqwest::Client::new();
    let request = apply_auth_to_request(
        client.get("https://bugzilla.example/rest/bug/1"),
        None,
        Some("secret-key"),
    )
    .build()
    .unwrap();

    assert_eq!(request.url().query(), Some("Bugzilla_api_key=secret-key"));
    assert!(request.headers().get(AUTH_HEADER_NAME).is_none());
}

#[test]
fn apply_auth_to_request_without_auth_leaves_request_unchanged() {
    let client = reqwest::Client::new();
    let request = apply_auth_to_request(
        client.get("https://bugzilla.example/rest/bug/1"),
        None,
        None,
    )
    .build()
    .unwrap();

    assert_eq!(
        request.url().as_str(),
        "https://bugzilla.example/rest/bug/1"
    );
    assert!(request.headers().get(AUTH_HEADER_NAME).is_none());
}

#[test]
fn apply_auth_header_method_adds_header() {
    let client = reqwest::Client::new();
    let request = apply_auth(
        client.get("https://bugzilla.example/rest/bug/1"),
        "header-key",
        AuthMethod::Header,
    )
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        request.headers().get(AUTH_HEADER_NAME).unwrap(),
        "header-key"
    );
}

#[test]
fn apply_auth_query_param_method_adds_query() {
    let client = reqwest::Client::new();
    let request = apply_auth(
        client.get("https://bugzilla.example/rest/bug/1"),
        "query-key",
        AuthMethod::QueryParam,
    )
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(request.url().query(), Some("Bugzilla_api_key=query-key"));
}

#[test]
fn apply_auth_header_method_rejects_invalid_value() {
    let client = reqwest::Client::new();
    let err = apply_auth(
        client.get("https://bugzilla.example/rest/bug/1"),
        "bad\nkey",
        AuthMethod::Header,
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid header characters"));
}

#[test]
fn looks_like_tls_error_matches_cert_keyword() {
    assert!(looks_like_tls_error("certificate verify failed"));
}

#[test]
fn looks_like_tls_error_matches_ssl_keyword() {
    assert!(looks_like_tls_error("SSL handshake failure"));
}

#[test]
fn looks_like_tls_error_matches_tls_keyword() {
    assert!(looks_like_tls_error("TLS protocol error"));
}

#[test]
fn looks_like_tls_error_rejects_unrelated_message() {
    assert!(!looks_like_tls_error("connection refused"));
}

#[test]
fn is_connect_tls_error_true_when_connect_and_tls_keyword() {
    assert!(is_connect_tls_error(true, "tls handshake failed"));
}

#[test]
fn is_connect_tls_error_false_when_not_connect() {
    assert!(!is_connect_tls_error(false, "tls handshake failed"));
}

#[test]
fn is_connect_tls_error_false_without_tls_keyword() {
    assert!(!is_connect_tls_error(true, "connection refused"));
}

#[tokio::test]
async fn tls_hint_no_hint_for_non_tls_error() {
    // Connection-refused is not a TLS error — should return the message unchanged.
    let client = crate::tls::build_tls_client(&crate::tls::TlsConfig::default()).unwrap();
    let err = client
        .get("http://127.0.0.1:1/nope")
        .send()
        .await
        .unwrap_err();
    let result = tls_hint("connection failed", &err);
    assert_eq!(result, "connection failed");
}

#[test]
fn redact_api_key_redacts_simple_query_param() {
    let input = "error sending request for url (http://localhost:8090/rest/extensions?Bugzilla_api_key=SecretKey123)";
    let result = redact_api_key(input);
    assert!(
        !result.contains("SecretKey123"),
        "API key should be redacted: {result}"
    );
    assert!(
        result.contains("Bugzilla_api_key=[REDACTED]"),
        "should contain redacted placeholder: {result}"
    );
    assert!(
        result.contains("rest/extensions"),
        "path should be preserved: {result}"
    );
}

#[test]
fn redact_api_key_preserves_message_without_key() {
    let input = "connection refused";
    assert_eq!(redact_api_key(input), "connection refused");
}

#[test]
fn redact_api_key_handles_marker_at_string_start() {
    let input = "Bugzilla_api_key=secret";
    assert_eq!(redact_api_key(input), "Bugzilla_api_key=[REDACTED]");
}

#[test]
fn redact_api_key_preserves_subsequent_query_params() {
    let input = "error for url (http://host/rest/bug?Bugzilla_api_key=secret&include_fields=id)";
    let result = redact_api_key(input);
    assert!(
        !result.contains("secret"),
        "API key should be redacted: {result}"
    );
    assert!(
        result.contains("&include_fields=id"),
        "other params should be preserved: {result}"
    );
}

// ── Timeout + retry helpers (#311) ──────────────────────────────────

#[test]
fn is_retryable_status_covers_429_and_5xx_only() {
    assert!(is_retryable_status(429));
    assert!(is_retryable_status(500));
    assert!(is_retryable_status(503));
    assert!(is_retryable_status(599));
    assert!(!is_retryable_status(200));
    assert!(!is_retryable_status(400));
    assert!(!is_retryable_status(401));
    assert!(!is_retryable_status(404));
    assert!(!is_retryable_status(418));
}

#[test]
fn should_retry_status_gates_5xx_on_safety() {
    // 429 is always retryable (rate-limited, not processed).
    assert!(should_retry_status(429, false));
    assert!(should_retry_status(429, true));
    // 5xx only for safe (GET/HEAD) requests; a write 500 may have been applied.
    assert!(should_retry_status(503, true));
    assert!(!should_retry_status(503, false));
    assert!(should_retry_status(500, true));
    assert!(!should_retry_status(500, false));
    // Non-transient statuses are never retried regardless of safety.
    assert!(!should_retry_status(404, true));
    assert!(!should_retry_status(200, true));
}

#[test]
fn backoff_delay_grows_exponentially_and_caps() {
    let d0 = backoff_delay(0, None);
    let d1 = backoff_delay(1, None);
    let d2 = backoff_delay(2, None);
    assert!(d1 > d0, "attempt 1 should wait longer than attempt 0");
    assert!(d2 > d1, "attempt 2 should wait longer than attempt 1");
    // Far-out attempts are capped, never unbounded.
    assert!(backoff_delay(20, None) <= std::time::Duration::from_secs(30));
}

#[test]
fn backoff_delay_honors_retry_after_when_longer() {
    // A long server Retry-After overrides the short exponential base.
    let ra = std::time::Duration::from_secs(10);
    assert_eq!(backoff_delay(0, Some(ra)), ra);
    // But a Retry-After is still capped.
    let huge = std::time::Duration::from_secs(9999);
    assert!(backoff_delay(0, Some(huge)) <= std::time::Duration::from_secs(30));
    // A tiny Retry-After never shortens the exponential base.
    let tiny = std::time::Duration::from_millis(1);
    assert!(backoff_delay(3, Some(tiny)) >= backoff_delay(3, None));
}

#[test]
fn parse_retry_after_reads_delta_seconds() {
    assert_eq!(
        parse_retry_after("5"),
        Some(std::time::Duration::from_secs(5))
    );
    assert_eq!(
        parse_retry_after("  12 "),
        Some(std::time::Duration::from_secs(12))
    );
}

#[test]
fn parse_retry_after_rejects_non_integer() {
    // HTTP-date form is unsupported (no date dep); falls back to backoff.
    assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None);
    assert_eq!(parse_retry_after("soon"), None);
    assert_eq!(parse_retry_after(""), None);
}

#[test]
fn resolve_timeout_secs_prefers_flag_then_env() {
    assert_eq!(resolve_timeout_secs(Some(15), Some("99")), Some(15));
    assert_eq!(resolve_timeout_secs(None, Some("99")), Some(99));
    assert_eq!(resolve_timeout_secs(None, None), None);
}

#[test]
fn resolve_timeout_secs_ignores_invalid_env() {
    assert_eq!(resolve_timeout_secs(None, Some("0")), None);
    assert_eq!(resolve_timeout_secs(None, Some("-3")), None);
    assert_eq!(resolve_timeout_secs(None, Some("abc")), None);
}
