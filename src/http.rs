use std::fmt::Write as _;
use std::time::Duration;

/// Kept short (10s) to fail fast on unreachable servers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-request ceiling (30s) — covers large attachment downloads.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Bugzilla's non-standard auth header (not `Authorization`).
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Bugzilla's query-param auth key — used by servers that reject header auth.
pub(crate) const AUTH_QUERY_PARAM: &str = "Bugzilla_api_key";

/// Apply a pre-validated header value or query-param key to a request builder.
///
/// This is the shared auth-application primitive. Both the pre-client
/// [`apply_auth`] and [`crate::client::BugzillaClient::apply_auth`] delegate here.
pub(crate) fn apply_auth_to_request(
    builder: reqwest::RequestBuilder,
    header: Option<&reqwest::header::HeaderValue>,
    query_key: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(val) = header {
        builder.header(AUTH_HEADER_NAME, val.clone())
    } else if let Some(key) = query_key {
        builder.query(&[(AUTH_QUERY_PARAM, key)])
    } else {
        builder
    }
}

/// Apply auth credentials to a request builder based on the configured method.
///
/// This is the fallible version used during auth detection (before a
/// [`crate::client::BugzillaClient`] is constructed). Returns `Err` if the
/// API key contains characters invalid for HTTP headers.
pub(crate) fn apply_auth(
    builder: reqwest::RequestBuilder,
    api_key: &str,
    method: crate::types::AuthMethod,
) -> crate::error::Result<reqwest::RequestBuilder> {
    match method {
        crate::types::AuthMethod::Header => {
            let val = reqwest::header::HeaderValue::from_str(api_key).map_err(|_| {
                crate::error::BzrError::config("API key contains invalid header characters")
            })?;
            Ok(apply_auth_to_request(builder, Some(&val), None))
        }
        crate::types::AuthMethod::QueryParam => {
            Ok(apply_auth_to_request(builder, None, Some(api_key)))
        }
    }
}

/// Build a shared HTTP client with standard timeout configuration.
///
/// When `tls_insecure` is true, the client accepts any TLS certificate
/// (self-signed, expired, wrong hostname). A warning is emitted at the
/// call site to make this visible to the user.
pub(crate) fn build_http_client(
    tls_insecure: bool,
) -> std::result::Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .danger_accept_invalid_certs(tls_insecure)
        .build()
}

/// Check if a reqwest error looks like a TLS certificate verification failure.
pub(crate) fn is_tls_cert_error(err: &reqwest::Error) -> bool {
    if !err.is_connect() {
        return false;
    }
    let msg = format!("{err:#}");
    let lower = msg.to_ascii_lowercase();
    lower.contains("certificate")
        || lower.contains("cert")
        || lower.contains("ssl")
        || lower.contains("tls")
        || lower.contains("invalid peer certificate")
}

/// Append a `--tls-insecure` hint to a message when a TLS certificate
/// error is detected, returning the enriched string.
pub(crate) fn tls_hint(base_msg: &str, err: &reqwest::Error) -> String {
    let mut msg = base_msg.to_string();
    if is_tls_cert_error(err) {
        let _ = write!(
            msg,
            "\n  hint: if this server uses a self-signed certificate or sits \
             behind a TLS-intercepting proxy, re-run:\n    \
             bzr config set-server <NAME> ... --tls-insecure"
        );
    }
    msg
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_http_client_succeeds() {
        let client = build_http_client(false);
        assert!(client.is_ok());
    }

    #[test]
    fn build_http_client_insecure_succeeds() {
        let client = build_http_client(true);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn tls_hint_no_hint_for_non_tls_error() {
        // Connection-refused is not a TLS error — should return the message unchanged.
        let client = build_http_client(false).unwrap();
        let err = client
            .get("http://127.0.0.1:1/nope")
            .send()
            .await
            .unwrap_err();
        let result = tls_hint("connection failed", &err);
        assert_eq!(result, "connection failed");
    }
}
