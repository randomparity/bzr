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

// The cases below arrived with #505, which routed `HttpStatus { body }` — a
// complete, untruncated server response — through this helper. The earlier
// callers both guaranteed a single-line, single-URL string: reqwest's error
// chain is one line, and `format_body_preview` collapses whitespace before
// calling. A raw body carries neither guarantee.

#[test]
fn redact_api_key_redacts_every_occurrence() {
    // A proxy or CGI::Carp 5xx page echoes the request URI more than once —
    // in the <title> and again in the prose.
    let input = "<title>500 /rest/bug?Bugzilla_api_key=SECRET1</title>\
                 The URL /rest/bug?Bugzilla_api_key=SECRET2 failed";
    let result = redact_api_key(input);
    assert!(!result.contains("SECRET1"), "first key leaked: {result}");
    assert!(!result.contains("SECRET2"), "second key leaked: {result}");
    assert_eq!(
        result.matches("Bugzilla_api_key=[REDACTED]").count(),
        2,
        "{result}"
    );
}

#[test]
fn redact_api_key_stops_at_a_line_break() {
    let input = "error\nBugzilla_api_key=secret\nnext line here";
    assert_eq!(
        redact_api_key(input),
        "error\nBugzilla_api_key=[REDACTED]\nnext line here"
    );
}

#[test]
fn redact_api_key_stops_at_markup_and_quote_delimiters() {
    let input = "<a href=\"/rest/bug?Bugzilla_api_key=secret\">link</a>";
    assert_eq!(
        redact_api_key(input),
        "<a href=\"/rest/bug?Bugzilla_api_key=[REDACTED]\">link</a>"
    );
}

#[test]
fn redact_api_key_handles_empty_value_at_end_of_string() {
    assert_eq!(
        redact_api_key("no key supplied: Bugzilla_api_key="),
        "no key supplied: Bugzilla_api_key=[REDACTED]"
    );
}

#[test]
fn redact_api_key_handles_multibyte_text_around_the_marker() {
    // A raw body is arbitrary text; slicing on a non-char boundary would panic.
    let input = "naïve café ?Bugzilla_api_key=secret&é=1 … déjà vu";
    let result = redact_api_key(input);
    assert_eq!(
        result,
        "naïve café ?Bugzilla_api_key=[REDACTED]&é=1 … déjà vu"
    );
}

#[test]
fn redact_api_key_is_idempotent() {
    let once = redact_api_key("url?Bugzilla_api_key=secret&id=1");
    assert_eq!(redact_api_key(&once), once);
}
