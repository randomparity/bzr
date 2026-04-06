use std::fmt;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BzrError {
    #[error("HTTP request failed: {}", format_http_error(.0))]
    Http(#[from] reqwest::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Bugzilla API error: {message} (code {code})")]
    Api { code: i64, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("XML-RPC error: {0}")]
    XmlRpc(String),

    #[error("{resource} not found: {id}")]
    NotFound { resource: &'static str, id: String },

    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("{0}")]
    InputValidation(String),

    #[error("Failed to parse response: {0}")]
    Deserialize(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Data integrity error: {0}")]
    DataIntegrity(String),

    #[error("batch update: {succeeded} succeeded, {failed} failed")]
    BatchPartialFailure { succeeded: usize, failed: usize },

    #[error("keyring error: {0}")]
    Keyring(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BzrError>;

// Error type constants for type-safe error classification
const ERROR_TYPE_CONFIG: &str = "config";
const ERROR_TYPE_API: &str = "api";
const ERROR_TYPE_HTTP: &str = "http";
const ERROR_TYPE_IO: &str = "io";
const ERROR_TYPE_NOT_FOUND: &str = "not_found";
const ERROR_TYPE_INPUT: &str = "input";
const ERROR_TYPE_DESERIALIZE: &str = "deserialize";
const ERROR_TYPE_AUTH: &str = "auth";
const ERROR_TYPE_DATA_INTEGRITY: &str = "data_integrity";
const ERROR_TYPE_BATCH_PARTIAL_FAILURE: &str = "batch_partial_failure";
const ERROR_TYPE_KEYRING: &str = "keyring";
const ERROR_TYPE_OTHER: &str = "other";

// Exit code constants
const EXIT_CODE_OTHER: i32 = 1;
const EXIT_CODE_NOT_FOUND: i32 = 2;
const EXIT_CODE_CONFIG: i32 = 3;
const EXIT_CODE_API: i32 = 4;
const EXIT_CODE_HTTP: i32 = 5;
const EXIT_CODE_IO: i32 = 6;
const EXIT_CODE_INPUT: i32 = 7;
const EXIT_CODE_DESERIALIZE: i32 = 8;
const EXIT_CODE_AUTH: i32 = 9;
const EXIT_CODE_DATA_INTEGRITY: i32 = 10;
const EXIT_CODE_BATCH_PARTIAL_FAILURE: i32 = 11;
const EXIT_CODE_KEYRING: i32 = 12;

/// Bugzilla internal server error code (HTTP 500 with code 100500).
/// Used for retry logic in hybrid mode when extensions crash.
pub const BUGZILLA_INTERNAL_ERROR: i64 = 100_500;

/// Format a reqwest error for display: redact API keys and add TLS hints.
fn format_http_error(err: &reqwest::Error) -> String {
    let mut msg = redact_api_key(&err.to_string());
    if crate::http::is_tls_cert_error(err) {
        msg.push_str(
            "\n  hint: if this server uses a self-signed certificate or sits \
             behind a TLS-intercepting proxy, re-run:\n    \
             bzr config set-server <NAME> ... --tls-insecure",
        );
    }
    msg
}

fn redact_api_key(msg: &str) -> String {
    const MARKER: &str = "Bugzilla_api_key=";
    if let Some(idx) = msg.find(MARKER) {
        let prefix = &msg[..idx + MARKER.len()];
        // Find the end of the key value (next & or ) or end of string)
        let rest = &msg[idx + MARKER.len()..];
        let end = rest.find(['&', ')', ' ']).unwrap_or(rest.len());
        format!("{prefix}[REDACTED]{}", &rest[end..])
    } else {
        msg.to_string()
    }
}

impl BzrError {
    pub fn config(msg: impl fmt::Display) -> Self {
        BzrError::Config(msg.to_string())
    }

    /// Returns `true` for transport-level failures that may succeed on retry
    /// via a different protocol (e.g. XML-RPC fallback in Hybrid mode).
    /// Domain errors like `Auth`, `NotFound`, and `Config` are not retriable.
    pub fn is_transport_failure(&self) -> bool {
        matches!(
            self,
            BzrError::Http(_) | BzrError::HttpStatus { .. } | BzrError::XmlRpc(_)
        )
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => {
                EXIT_CODE_CONFIG
            }
            BzrError::Api { .. } | BzrError::XmlRpc(_) => EXIT_CODE_API,
            BzrError::Http(_) | BzrError::HttpStatus { .. } => EXIT_CODE_HTTP,
            BzrError::Io(_) => EXIT_CODE_IO,
            BzrError::NotFound { .. } => EXIT_CODE_NOT_FOUND,
            BzrError::InputValidation(_) => EXIT_CODE_INPUT,
            BzrError::Deserialize(_) => EXIT_CODE_DESERIALIZE,
            BzrError::Auth(_) => EXIT_CODE_AUTH,
            BzrError::DataIntegrity(_) => EXIT_CODE_DATA_INTEGRITY,
            BzrError::BatchPartialFailure { .. } => EXIT_CODE_BATCH_PARTIAL_FAILURE,
            BzrError::Keyring(_) => EXIT_CODE_KEYRING,
            BzrError::Other(_) => EXIT_CODE_OTHER,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => {
                ERROR_TYPE_CONFIG
            }
            BzrError::Api { .. } | BzrError::XmlRpc(_) => ERROR_TYPE_API,
            BzrError::Http(_) | BzrError::HttpStatus { .. } => ERROR_TYPE_HTTP,
            BzrError::Io(_) => ERROR_TYPE_IO,
            BzrError::NotFound { .. } => ERROR_TYPE_NOT_FOUND,
            BzrError::InputValidation(_) => ERROR_TYPE_INPUT,
            BzrError::Deserialize(_) => ERROR_TYPE_DESERIALIZE,
            BzrError::Auth(_) => ERROR_TYPE_AUTH,
            BzrError::DataIntegrity(_) => ERROR_TYPE_DATA_INTEGRITY,
            BzrError::BatchPartialFailure { .. } => ERROR_TYPE_BATCH_PARTIAL_FAILURE,
            BzrError::Keyring(_) => ERROR_TYPE_KEYRING,
            BzrError::Other(_) => ERROR_TYPE_OTHER,
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_config() {
        let err = BzrError::Config("bad config".into());
        assert_eq!(err.exit_code(), 3);
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
    fn exit_code_other() {
        let err = BzrError::Other("something went wrong".into());
        assert_eq!(err.exit_code(), 1);
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
    fn error_type_other() {
        let err = BzrError::Other("x".into());
        assert_eq!(err.error_type(), "other");
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
    fn sanitize_http_error_redacts_api_key() {
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
    fn sanitize_http_error_preserves_message_without_key() {
        let input = "connection refused";
        let result = redact_api_key(input);
        assert_eq!(result, "connection refused");
    }

    #[test]
    fn sanitize_http_error_handles_key_with_other_params() {
        let input =
            "error for url (http://host/rest/bug?Bugzilla_api_key=secret&include_fields=id)";
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

    #[test]
    fn exit_code_keyring() {
        let err = BzrError::Keyring("keychain locked".into());
        assert_eq!(err.exit_code(), 12);
        assert_eq!(err.error_type(), "keyring");
        assert_eq!(err.to_string(), "keyring error: keychain locked");
    }
}
