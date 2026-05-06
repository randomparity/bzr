use std::time::Duration;

/// Kept short (10s) to fail fast on unreachable servers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-request ceiling (30s) — covers large attachment downloads.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Cap applied to opportunistic XML-RPC retries triggered when a primary
/// REST request succeeded with no rows. The full `REQUEST_TIMEOUT` is too
/// generous here: REST already returned an answer, and a slow XML-RPC
/// fallback shouldn't make the user pay 30s for a retry that may not
/// improve the result. 8s is enough for a healthy `Bug.search` against a
/// large database while still failing fast on truly unresponsive servers.
pub(crate) const XMLRPC_FALLBACK_TIMEOUT: Duration = Duration::from_secs(8);

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

/// Redact a Bugzilla API key value out of a string for safe display.
///
/// Looks for the literal `Bugzilla_api_key=` marker (the query-param
/// form Bugzilla uses) and replaces the value up to the next `&`,
/// `)`, or space with `[REDACTED]`. If the marker is absent the input
/// is returned unchanged.
pub(crate) fn redact_api_key(msg: &str) -> String {
    let marker = format!("{AUTH_QUERY_PARAM}=");
    if let Some(idx) = msg.find(&marker) {
        let prefix = &msg[..idx + marker.len()];
        let rest = &msg[idx + marker.len()..];
        let end = rest.find(['&', ')', ' ']).unwrap_or(rest.len());
        format!("{prefix}[REDACTED]{}", &rest[end..])
    } else {
        msg.to_string()
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
