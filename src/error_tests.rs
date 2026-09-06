#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn exit_code_config() {
    let err = BzrError::Config("bad config".into());
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn mid_air_collision_has_distinct_exit_code_and_type() {
    let err = BzrError::MidAirCollision {
        id: 42,
        expected: "2026-06-19T10:00:00Z".into(),
        actual: "2026-06-19T12:00:00Z".into(),
    };
    assert_eq!(err.exit_code(), 14);
    assert_eq!(err.error_type(), "collision");
    let msg = err.to_string();
    assert!(
        msg.contains("42") && msg.contains("mid-air collision"),
        "{msg}"
    );
}

#[test]
fn exit_code_api() {
    let err = BzrError::Api {
        code: 101,
        message: "Invalid Bug ID".into(),
    };
    assert_eq!(err.exit_code(), 4);
}

#[test]
fn exit_code_io() {
    let err = BzrError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ));
    assert_eq!(err.exit_code(), 6);
}

#[test]
fn exit_code_toml_parse() {
    let toml_err: std::result::Result<toml::Value, _> = toml::from_str("{{bad");
    let err = BzrError::TomlParse(toml_err.unwrap_err());
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn error_type_config() {
    let err = BzrError::Config("x".into());
    assert_eq!(err.error_type(), "config");
}

#[test]
fn error_type_api() {
    let err = BzrError::Api {
        code: 1,
        message: "x".into(),
    };
    assert_eq!(err.error_type(), "api");
}

#[test]
fn error_type_io() {
    let err = BzrError::Io(std::io::Error::other("x"));
    assert_eq!(err.error_type(), "io");
}

#[test]
fn exit_code_not_found() {
    let err = BzrError::NotFound {
        resource: "bug",
        id: "42".into(),
    };
    assert_eq!(err.exit_code(), 2);
    assert_eq!(err.error_type(), "not_found");
    assert_eq!(err.to_string(), "bug not found: 42");
}

#[test]
fn error_type_toml_parse() {
    let toml_err: std::result::Result<toml::Value, _> = toml::from_str("{{bad");
    let err = BzrError::TomlParse(toml_err.unwrap_err());
    assert_eq!(err.error_type(), "config");
}

#[test]
fn exit_code_http_status() {
    let err = BzrError::HttpStatus {
        status: 500,
        body: "internal error".into(),
    };
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.error_type(), "http");
    assert_eq!(err.to_string(), "HTTP 500: internal error");
}

#[test]
fn exit_code_input_validation() {
    let err = BzrError::input("bad flag".into());
    assert_eq!(err.exit_code(), 7);
    assert_eq!(err.error_type(), "input");
    assert_eq!(err.to_string(), "bad flag");
}

#[test]
fn exit_code_deserialize() {
    let err = BzrError::Deserialize("invalid JSON".into());
    assert_eq!(err.exit_code(), 8);
    assert_eq!(err.error_type(), "deserialize");
    assert_eq!(err.to_string(), "Failed to parse response: invalid JSON");
}

#[test]
fn exit_code_auth() {
    let err = BzrError::Auth("invalid API key".into());
    assert_eq!(err.exit_code(), 9);
    assert_eq!(err.error_type(), "auth");
    assert_eq!(err.to_string(), "Authentication error: invalid API key");
}

#[test]
fn exit_code_data_integrity() {
    let err = BzrError::DataIntegrity("attachment has no data".into());
    assert_eq!(err.exit_code(), 10);
    assert_eq!(err.error_type(), "data_integrity");
}

#[test]
fn exit_code_batch_partial_failure() {
    let err = BzrError::BatchPartialFailure {
        succeeded: 3,
        failed: 2,
    };
    assert_eq!(err.exit_code(), 11);
    assert_eq!(err.error_type(), "batch_partial_failure");
    assert_eq!(err.to_string(), "batch update: 3 succeeded, 2 failed");
}

#[test]
fn exit_code_keyring() {
    let err = BzrError::Keyring("keychain locked".into());
    assert_eq!(err.exit_code(), 12);
    assert_eq!(err.error_type(), "keyring");
    assert_eq!(err.to_string(), "keyring error: keychain locked");
}

#[test]
fn exit_code_pin_mismatch() {
    let err = BzrError::PinMismatch {
        server: "test".into(),
        expected: "sha256//old".into(),
        actual: "sha256//new".into(),
    };
    assert_eq!(err.exit_code(), 13);
    assert_eq!(err.error_type(), "tls");
    assert!(err.to_string().contains("pin mismatch"));
}

#[test]
fn exit_code_issuer_changed() {
    let err = BzrError::IssuerChanged {
        server: "test".into(),
        expected_issuer: "CN=Good CA".into(),
        actual_issuer: "CN=Evil CA".into(),
    };
    assert_eq!(err.exit_code(), 13);
    assert_eq!(err.error_type(), "tls");
    assert!(err.to_string().contains("MITM"));
}

/// reqwest's `Display` omits the source chain (e.g. "connection refused").
/// Verify that `format_http_error` walks the chain so the user sees the
/// actual cause, not just "error sending request for url (URL)".
#[tokio::test]
async fn format_http_error_includes_source_chain() {
    let client = reqwest::Client::builder().build().unwrap();
    // Connect to a port that is almost certainly not listening.
    let err = client
        .get("http://127.0.0.1:1/unreachable")
        .send()
        .await
        .unwrap_err();

    // reqwest Display: only kind + URL, no cause
    let display_only = err.to_string();
    assert!(
        display_only.contains("error sending request"),
        "expected reqwest error kind: {display_only}"
    );

    let formatted = format_http_error(&err);
    // Our formatter must include the underlying OS-level cause
    assert!(
        formatted.len() > display_only.len(),
        "format_http_error should include source chain, got: {formatted}"
    );
    // The source chain should mention connection-level detail
    assert!(
        formatted.contains("connect") || formatted.contains("refused") || formatted.contains("tcp"),
        "expected connection-level detail in: {formatted}"
    );
}

#[test]
fn is_permissive_bug_view_error_true_for_notfound() {
    let err = BzrError::NotFound {
        resource: "bug",
        id: "123".into(),
    };
    assert!(err.is_permissive_bug_view_error());
}

#[test]
fn is_permissive_bug_view_error_true_for_api_100() {
    let err = BzrError::Api {
        code: 100,
        message: "invalid alias".into(),
    };
    assert!(err.is_permissive_bug_view_error());
}

#[test]
fn is_permissive_bug_view_error_true_for_api_101() {
    let err = BzrError::Api {
        code: 101,
        message: "invalid bug id".into(),
    };
    assert!(err.is_permissive_bug_view_error());
}

#[test]
fn is_permissive_bug_view_error_true_for_api_102() {
    let err = BzrError::Api {
        code: 102,
        message: "access denied".into(),
    };
    assert!(err.is_permissive_bug_view_error());
}

#[test]
fn is_permissive_bug_view_error_false_for_api_session_codes() {
    for code in [32000_i64, 32610, 100_500, 50_001] {
        let err = BzrError::Api {
            code,
            message: "session-wide".into(),
        };
        assert!(
            !err.is_permissive_bug_view_error(),
            "code {code} should NOT be per-resource"
        );
    }
}

#[test]
fn is_permissive_bug_view_error_false_for_transport_and_auth() {
    let err = BzrError::HttpStatus {
        status: 500,
        body: String::new(),
    };
    assert!(!err.is_permissive_bug_view_error());

    let err = BzrError::Auth("session expired".into());
    assert!(!err.is_permissive_bug_view_error());

    let err = BzrError::XmlRpc("transport".into());
    assert!(!err.is_permissive_bug_view_error());
}

fn detail(err: &BzrError) -> serde_json::Map<String, serde_json::Value> {
    err.structured_detail()
}

#[test]
fn structured_detail_collision_carries_retry_tokens() {
    let err = BzrError::MidAirCollision {
        id: 42,
        expected: "TOKEN-A".into(),
        actual: "2026-06-19T12:00:00Z".into(),
    };
    let d = detail(&err);
    assert_eq!(
        d.get("bug_id").and_then(serde_json::Value::as_u64),
        Some(42)
    );
    assert_eq!(
        d.get("last_change_time")
            .and_then(serde_json::Value::as_str),
        Some("2026-06-19T12:00:00Z")
    );
    assert_eq!(
        d.get("if_match_token").and_then(serde_json::Value::as_str),
        Some("TOKEN-A")
    );
}

#[test]
fn structured_detail_input_field_carries_field_and_value() {
    let err = BzrError::input_field(
        "deadline: 'x' is not a valid date".into(),
        "deadline",
        Some("x".into()),
    );
    let d = detail(&err);
    assert_eq!(
        d.get("field").and_then(serde_json::Value::as_str),
        Some("deadline")
    );
    assert_eq!(
        d.get("value").and_then(serde_json::Value::as_str),
        Some("x")
    );
}

#[test]
fn structured_detail_plain_input_has_no_attribution() {
    let err = BzrError::input("bad flag".into());
    let d = detail(&err);
    assert!(!d.contains_key("field"), "{d:?}");
    assert!(!d.contains_key("value"), "{d:?}");
}

#[test]
fn structured_detail_input_field_without_value_omits_value() {
    let err = BzrError::input_field("product is required".into(), "product", None);
    let d = detail(&err);
    assert_eq!(
        d.get("field").and_then(serde_json::Value::as_str),
        Some("product")
    );
    assert!(!d.contains_key("value"), "value absent when unknown: {d:?}");
}

#[test]
fn structured_detail_not_found_carries_resource_and_identifier() {
    let err = BzrError::NotFound {
        resource: "bug",
        id: "999".into(),
    };
    let d = detail(&err);
    assert_eq!(
        d.get("resource").and_then(serde_json::Value::as_str),
        Some("bug")
    );
    assert_eq!(
        d.get("identifier").and_then(serde_json::Value::as_str),
        Some("999")
    );
}

#[test]
fn structured_detail_http_status_carries_status() {
    let err = BzrError::HttpStatus {
        status: 404,
        body: "Not Found".into(),
    };
    let d = detail(&err);
    assert_eq!(
        d.get("status").and_then(serde_json::Value::as_u64),
        Some(404)
    );
}

#[test]
fn structured_detail_api_carries_api_code() {
    let err = BzrError::Api {
        code: 101,
        message: "Invalid Bug ID".into(),
    };
    let d = detail(&err);
    assert_eq!(
        d.get("api_code").and_then(serde_json::Value::as_i64),
        Some(101)
    );
}

#[test]
fn structured_detail_batch_partial_failure_carries_counts() {
    let err = BzrError::BatchPartialFailure {
        succeeded: 3,
        failed: 2,
    };
    let d = detail(&err);
    assert_eq!(
        d.get("succeeded").and_then(serde_json::Value::as_u64),
        Some(3)
    );
    assert_eq!(d.get("failed").and_then(serde_json::Value::as_u64), Some(2));
}

#[test]
fn structured_detail_pin_mismatch_carries_server_and_pins() {
    let err = BzrError::PinMismatch {
        server: "bugzilla.example".into(),
        expected: "AAAA".into(),
        actual: "BBBB".into(),
    };
    let d = detail(&err);
    assert_eq!(
        d.get("server").and_then(serde_json::Value::as_str),
        Some("bugzilla.example")
    );
    assert_eq!(
        d.get("expected").and_then(serde_json::Value::as_str),
        Some("AAAA")
    );
    assert_eq!(
        d.get("actual").and_then(serde_json::Value::as_str),
        Some("BBBB")
    );
}

#[test]
fn structured_detail_never_emits_reserved_keys() {
    let errs = [
        BzrError::input_field("m".into(), "f", Some("v".into())),
        BzrError::NotFound {
            resource: "bug",
            id: "1".into(),
        },
        BzrError::HttpStatus {
            status: 404,
            body: String::new(),
        },
        BzrError::Api {
            code: 1,
            message: "m".into(),
        },
        BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 1,
        },
        BzrError::MidAirCollision {
            id: 1,
            expected: "a".into(),
            actual: "b".into(),
        },
        BzrError::IssuerChanged {
            server: "s".into(),
            expected_issuer: "a".into(),
            actual_issuer: "b".into(),
        },
        BzrError::Config("c".into()),
    ];
    for err in &errs {
        let d = detail(err);
        for reserved in ["type", "message", "exit_code"] {
            assert!(
                !d.contains_key(reserved),
                "{} leaked reserved key {reserved}",
                err.error_type()
            );
        }
    }
}

/// A server that quotes the request URL back in its error text echoes the
/// credential with it. Both variants carry server-supplied strings, so both
/// redact at the `Display` seam every output format renders through.
#[test]
fn api_display_redacts_api_key_echoed_by_the_server() {
    let err = BzrError::Api {
        code: 32000,
        message: "bad request: /rest/bug?Bugzilla_api_key=SUPERSECRET&id=1".into(),
    };
    let msg = err.to_string();
    assert!(!msg.contains("SUPERSECRET"), "key leaked in Display: {msg}");
    assert!(msg.contains("Bugzilla_api_key=[REDACTED]"), "{msg}");
    assert!(msg.contains("code 32000"), "code must survive: {msg}");
}

#[test]
fn clearing_redaction_context_before_display_disables_bare_key_masking() {
    let _guard = crate::bugzilla_auth::active_api_key_test_guard(Some("SUPERSECRET"));
    super::clear_error_redaction_context();
    let err = BzrError::Api {
        code: 32000,
        message: "invalid SUPERSECRET".into(),
    };

    assert!(err.to_string().contains("SUPERSECRET"));
}

#[test]
fn http_status_display_redacts_api_key_echoed_by_the_server() {
    let err = BzrError::HttpStatus {
        status: 401,
        body: "unauthorized for /rest/bug?Bugzilla_api_key=SUPERSECRET".into(),
    };
    let msg = err.to_string();
    assert!(!msg.contains("SUPERSECRET"), "key leaked in Display: {msg}");
    assert!(msg.contains("Bugzilla_api_key=[REDACTED]"), "{msg}");
    assert!(msg.starts_with("HTTP 401: "), "status must survive: {msg}");
}

/// The redaction is marker-driven: text with no `Bugzilla_api_key=` marker
/// must render byte-for-byte as before, so ordinary server errors are not
/// mangled.
#[test]
fn display_leaves_server_text_without_the_marker_unchanged() {
    assert_eq!(
        BzrError::Api {
            code: 101,
            message: "Invalid Bug ID".into(),
        }
        .to_string(),
        "Bugzilla API error: Invalid Bug ID (code 101)"
    );
    assert_eq!(
        BzrError::HttpStatus {
            status: 503,
            body: "Service Unavailable".into(),
        }
        .to_string(),
        "HTTP 503: Service Unavailable"
    );
}

/// `structured_detail` publishes the machine-readable keys for these variants,
/// and today emits only `api_code` / `status` — so this passes without the
/// `Display` fix and proves nothing about it. It is a forward guard: a future
/// detail key carrying the raw server text would re-leak on `--json` what
/// `Display` masks.
#[test]
fn structured_detail_does_not_republish_raw_server_text() {
    let leaky = "Bugzilla_api_key=SUPERSECRET";
    for err in [
        BzrError::Api {
            code: 32000,
            message: format!("bad request: {leaky}"),
        },
        BzrError::HttpStatus {
            status: 401,
            body: format!("unauthorized: {leaky}"),
        },
    ] {
        let rendered = serde_json::to_string(&err.structured_detail()).unwrap();
        assert!(
            !rendered.contains("SUPERSECRET"),
            "{} leaked the key via structured_detail: {rendered}",
            err.error_type()
        );
    }
}

#[test]
fn unsupported_server_capability_has_distinct_exit_code_and_type() {
    let err = BzrError::UnsupportedServerCapability {
        capability: "RedHat".to_string(),
        operation: "saved search 'triage'".to_string(),
        detail: "server does not implement the Red Hat saved-search extension".to_string(),
    };
    assert_eq!(err.exit_code(), 15);
    assert_eq!(err.error_type(), "unsupported_server_capability");
    assert_eq!(
        err.structured_detail()
            .get("capability")
            .and_then(|v| v.as_str()),
        Some("RedHat")
    );
    assert!(err.to_string().starts_with("saved search 'triage': "));
}
