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

/// Check if an error message string contains TLS-related keywords.
pub(crate) fn looks_like_tls_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("cert") || lower.contains("ssl") || lower.contains("tls")
}

/// Pure predicate underlying [`is_tls_cert_error`], split out so the
/// connect-and-TLS-keyword logic can be unit tested without a live
/// `reqwest::Error` (which has no public constructor). Also called
/// directly by `format_http_error` to avoid recomputing the error chain.
pub(crate) fn is_connect_tls_error(is_connect: bool, error_chain: &str) -> bool {
    is_connect && looks_like_tls_error(error_chain)
}

/// Check if a reqwest error looks like a TLS certificate verification failure.
pub(crate) fn is_tls_cert_error(err: &reqwest::Error) -> bool {
    is_connect_tls_error(err.is_connect(), &crate::error::format_error_chain(err))
}

/// Hint text appended to TLS certificate errors.
pub(crate) const TLS_HINT: &str =
    "\n  hint: to trust this server's certificate, re-run interactively,\n    \
     or pre-pin with:  bzr config set-server <NAME> --tls-pin-now\n    \
     or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>\n    \
     or skip verification: bzr config set-server <NAME> --tls-insecure";

/// Append a `--tls-insecure` hint to a message when a TLS certificate
/// error is detected, returning the enriched string.
pub(crate) fn tls_hint(base_msg: &str, err: &reqwest::Error) -> String {
    let mut msg = base_msg.to_string();
    if is_tls_cert_error(err) {
        msg.push_str(TLS_HINT);
    }
    msg
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
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
}
