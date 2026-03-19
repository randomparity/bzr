use std::time::Duration;

/// HTTP connection timeout for Bugzilla API requests.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// HTTP request timeout for Bugzilla API requests.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP header name for Bugzilla API key authentication.
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Query parameter name for Bugzilla API key authentication.
pub(crate) const AUTH_QUERY_PARAM: &str = "Bugzilla_api_key";

/// Build a shared HTTP client with standard timeout configuration.
pub(crate) fn build_http_client() -> std::result::Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
}
