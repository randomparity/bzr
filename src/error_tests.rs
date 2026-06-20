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
    let err = BzrError::InputValidation("bad flag".into());
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
fn is_bug_get_per_resource_true_for_notfound() {
    let err = BzrError::NotFound {
        resource: "bug",
        id: "123".into(),
    };
    assert!(err.is_bug_get_per_resource());
}

#[test]
fn is_bug_get_per_resource_true_for_api_100() {
    let err = BzrError::Api {
        code: 100,
        message: "invalid alias".into(),
    };
    assert!(err.is_bug_get_per_resource());
}

#[test]
fn is_bug_get_per_resource_true_for_api_101() {
    let err = BzrError::Api {
        code: 101,
        message: "invalid bug id".into(),
    };
    assert!(err.is_bug_get_per_resource());
}

#[test]
fn is_bug_get_per_resource_true_for_api_102() {
    let err = BzrError::Api {
        code: 102,
        message: "access denied".into(),
    };
    assert!(err.is_bug_get_per_resource());
}

#[test]
fn is_bug_get_per_resource_false_for_api_session_codes() {
    for code in [32000_i64, 32610, 100_500, 50_001] {
        let err = BzrError::Api {
            code,
            message: "session-wide".into(),
        };
        assert!(
            !err.is_bug_get_per_resource(),
            "code {code} should NOT be per-resource"
        );
    }
}

#[test]
fn is_bug_get_per_resource_false_for_transport_and_auth() {
    let err = BzrError::HttpStatus {
        status: 500,
        body: String::new(),
    };
    assert!(!err.is_bug_get_per_resource());

    let err = BzrError::Auth("session expired".into());
    assert!(!err.is_bug_get_per_resource());

    let err = BzrError::XmlRpc("transport".into());
    assert!(!err.is_bug_get_per_resource());
}
