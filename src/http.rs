use std::time::Duration;

/// Kept short (10s) to fail fast on unreachable servers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-request ceiling (30s) — covers large attachment downloads.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Bugzilla's non-standard auth header (not `Authorization`).
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Bugzilla's query-param auth key — used by servers that reject header auth.
pub(crate) const AUTH_QUERY_PARAM: &str = "Bugzilla_api_key";

/// Apply auth credentials to a request builder based on the configured method.
///
/// Returns `Err` if the API key contains characters invalid for HTTP headers
/// when using header-based auth. Callers that want best-effort auth should
/// handle the error (e.g. fall back to sending without auth).
pub(crate) fn apply_auth(
    builder: reqwest::RequestBuilder,
    api_key: &str,
    method: crate::types::AuthMethod,
) -> crate::error::Result<reqwest::RequestBuilder> {
    match method {
        crate::types::AuthMethod::Header => {
            let val = reqwest::header::HeaderValue::from_str(api_key)
                .map_err(|_| crate::error::BzrError::config("API key contains invalid header characters"))?;
            Ok(builder.header(AUTH_HEADER_NAME, val))
        }
        crate::types::AuthMethod::QueryParam => Ok(builder.query(&[(AUTH_QUERY_PARAM, api_key)])),
    }
}

/// Build a shared HTTP client with standard timeout configuration.
pub(crate) fn build_http_client() -> std::result::Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_http_client_succeeds() {
        let client = build_http_client();
        assert!(client.is_ok());
    }
}
