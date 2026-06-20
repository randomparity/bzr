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

    #[error("TLS pin mismatch for {server}: expected {expected}, got {actual}")]
    PinMismatch {
        server: String,
        expected: String,
        actual: String,
    },

    #[error(
        "TLS certificate issuer changed for {server}: expected \"{expected_issuer}\", \
         got \"{actual_issuer}\" — possible MITM attack"
    )]
    IssuerChanged {
        server: String,
        expected_issuer: String,
        actual_issuer: String,
    },

    #[error(
        "mid-air collision on bug {id}: it last changed at {actual}, not {expected} \
         as expected — someone modified it since; re-read the bug and retry"
    )]
    MidAirCollision {
        id: u64,
        expected: String,
        actual: String,
    },
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
const ERROR_TYPE_TLS: &str = "tls";
const ERROR_TYPE_COLLISION: &str = "collision";

// Exit code constants
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
const EXIT_CODE_TLS: i32 = 13;
const EXIT_CODE_COLLISION: i32 = 14;

/// Bugzilla internal server error code (HTTP 500 with code 100500).
/// Used for retry logic in hybrid mode when extensions crash.
pub const BUGZILLA_INTERNAL_ERROR: i64 = 100_500;

/// Bugzilla `Bug.get` per-bug fault codes.
/// 100: Invalid Bug Alias  ·  101: Invalid Bug ID  ·  102: Access Denied
/// Reference: <https://bugzilla.readthedocs.io/en/latest/api/core/v1/bug.html>
const BUG_GET_PER_RESOURCE_CODES: &[i64] = &[100, 101, 102];

/// Walk a `std::error::Error` source chain into a single string.
///
/// reqwest's `Display` only shows the error kind and URL, omitting the
/// underlying cause. This helper concatenates the full chain so callers
/// get actionable messages like "error sending request …: invalid peer
/// certificate: `UnknownIssuer`".
pub(crate) fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut full = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        full.push_str(": ");
        full.push_str(&cause.to_string());
        source = cause.source();
    }
    full
}

/// Format a reqwest error for display: redact API keys and add TLS hints.
fn format_http_error(err: &reqwest::Error) -> String {
    let chain = format_error_chain(err);
    let mut msg = crate::http::redact_api_key(&chain);
    if crate::http::is_connect_tls_error(err.is_connect(), &chain) {
        msg.push_str(crate::http::TLS_HINT);
    }
    msg
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

    /// Returns true for per-resource errors that may be suppressed
    /// under `bzr bug view --permissive` (one inaccessible bug among
    /// many). Session-level failures (transport, auth, security,
    /// uncategorized server faults) always bail.
    pub fn is_bug_get_per_resource(&self) -> bool {
        match self {
            BzrError::NotFound { .. } => true,
            BzrError::Api { code, .. } => BUG_GET_PER_RESOURCE_CODES.contains(code),
            _ => false,
        }
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
            BzrError::PinMismatch { .. } | BzrError::IssuerChanged { .. } => EXIT_CODE_TLS,
            BzrError::MidAirCollision { .. } => EXIT_CODE_COLLISION,
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
            BzrError::PinMismatch { .. } | BzrError::IssuerChanged { .. } => ERROR_TYPE_TLS,
            BzrError::MidAirCollision { .. } => ERROR_TYPE_COLLISION,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
