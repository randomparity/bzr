use std::fmt;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BzrError {
    #[error("HTTP request failed: {}", sanitize_http_error(.0))]
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

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BzrError>;

/// Strip API keys from reqwest error messages to avoid leaking credentials.
///
/// Reqwest includes the full URL (with query params) in its error Display.
/// When using query-param auth, this exposes the `Bugzilla_api_key` value.
fn sanitize_http_error(err: &reqwest::Error) -> String {
    sanitize_http_error_str(&err.to_string())
}

fn sanitize_http_error_str(msg: &str) -> String {
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
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => 3,
            BzrError::Api { .. } | BzrError::XmlRpc(_) => 4,
            BzrError::Http(_) | BzrError::HttpStatus { .. } => 5,
            BzrError::Io(_) => 6,
            BzrError::NotFound { .. } => 2,
            BzrError::InputValidation(_) => 7,
            BzrError::Deserialize(_) => 8,
            BzrError::Auth(_) => 9,
            BzrError::DataIntegrity(_) => 10,
            BzrError::Other(_) => 1,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => "config",
            BzrError::Api { .. } | BzrError::XmlRpc(_) => "api",
            BzrError::Http(_) | BzrError::HttpStatus { .. } => "http",
            BzrError::Io(_) => "io",
            BzrError::NotFound { .. } => "not_found",
            BzrError::InputValidation(_) => "input",
            BzrError::Deserialize(_) => "deserialize",
            BzrError::Auth(_) => "auth",
            BzrError::DataIntegrity(_) => "data_integrity",
            BzrError::Other(_) => "other",
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
        let result = sanitize_http_error_str(input);
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
        let result = sanitize_http_error_str(input);
        assert_eq!(result, "connection refused");
    }

    #[test]
    fn sanitize_http_error_handles_key_with_other_params() {
        let input =
            "error for url (http://host/rest/bug?Bugzilla_api_key=secret&include_fields=id)";
        let result = sanitize_http_error_str(input);
        assert!(
            !result.contains("secret"),
            "API key should be redacted: {result}"
        );
        assert!(
            result.contains("&include_fields=id"),
            "other params should be preserved: {result}"
        );
    }
}
