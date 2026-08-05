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

/// Every terminator, each proved by exact equality so the text that follows the
/// value is shown to survive. Dropping any one of these from
/// `ends_api_key_value` must redden a case here — otherwise the set drifts to
/// whatever a later reader assumes it is. The space case matters most:
/// `format_body_preview` collapses `\n\r\t` to spaces *before* calling, so
/// space termination is what keeps every body preview from over-consuming.
#[test]
fn redact_api_key_ends_the_value_at_each_terminator() {
    let cases = [
        ("GET ?Bugzilla_api_key=secret HTTP/1.1", "space"),
        ("a\nBugzilla_api_key=secret\nnext line", "newline"),
        ("a\r\nBugzilla_api_key=secret\r\nnext line", "crlf"),
        ("col\tBugzilla_api_key=secret\tnext col", "tab"),
        ("?Bugzilla_api_key=secret&id=1", "ampersand"),
        ("url (?Bugzilla_api_key=secret) failed", "close paren"),
        ("href=\"?Bugzilla_api_key=secret\">link", "double quote"),
        ("href='?Bugzilla_api_key=secret'>link", "single quote"),
        ("<?Bugzilla_api_key=secret<tag>", "less than"),
        ("v=?Bugzilla_api_key=secret>rest", "greater than"),
        ("?Bugzilla_api_key=secret#fragment", "hash"),
    ];
    for (input, terminator) in cases {
        assert_eq!(
            redact_api_key(input),
            input.replace("secret", "[REDACTED]"),
            "value not terminated by {terminator}"
        );
    }
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

/// The shapes that matter are the ones where `[REDACTED]` is not followed by a
/// terminator — at end of string, and where the original value was empty — since
/// those are where a second pass could consume the placeholder itself.
#[test]
fn redact_api_key_is_idempotent() {
    for input in [
        "url?Bugzilla_api_key=secret&id=1",
        "Bugzilla_api_key=secret",
        "Bugzilla_api_key=",
        "a\nBugzilla_api_key=k\nb",
        "no marker at all",
    ] {
        let once = redact_api_key(input);
        assert_eq!(redact_api_key(&once), once, "not idempotent for {input:?}");
    }
}

#[test]
fn redact_api_key_masks_every_bare_active_key_occurrence() {
    let _guard = active_api_key_test_guard(Some("configured-secret"));
    assert_eq!(
        redact_api_key("invalid configured-secret; configured-secret rejected"),
        "invalid [REDACTED]; [REDACTED] rejected"
    );
}

#[test]
fn redact_api_key_applies_the_eight_byte_bare_key_floor() {
    {
        let _guard = active_api_key_test_guard(Some("seven77"));
        assert_eq!(redact_api_key("invalid seven77"), "invalid seven77");
    }
    {
        let _guard = active_api_key_test_guard(Some("eight888"));
        assert_eq!(redact_api_key("invalid eight888"), "invalid [REDACTED]");
    }
}

#[test]
fn redact_api_key_masks_encoded_markers_at_any_value_length() {
    let _guard = active_api_key_test_guard(None);
    for marker in [
        "Bugzilla_api_key=",
        "Bugzilla_api_key%3D",
        "Bugzilla_api_key%3d",
    ] {
        assert_eq!(
            redact_api_key(&format!("error: {marker}x&next=1")),
            format!("error: {marker}[REDACTED]&next=1")
        );
    }
}

#[test]
fn active_api_key_test_guard_restores_prior_thread_state() {
    {
        let _guard = active_api_key_test_guard(Some("former-secret"));
        assert_eq!(redact_api_key("former-secret"), "[REDACTED]");
    }
    let _guard = active_api_key_test_guard(None);
    assert_eq!(redact_api_key("former-secret"), "former-secret");
}
