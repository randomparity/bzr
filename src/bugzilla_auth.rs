/// Bugzilla's non-standard auth header (not `Authorization`).
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Bugzilla's query-param auth key, used by servers that reject header auth.
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
/// This is the fallible version used during auth detection, before a
/// [`crate::client::BugzillaClient`] is constructed. Returns `Err` if the
/// API key contains characters invalid for HTTP headers.
pub(crate) fn apply_auth(
    builder: reqwest::RequestBuilder,
    api_key: &str,
    method: crate::types::common::AuthMethod,
) -> crate::error::Result<reqwest::RequestBuilder> {
    match method {
        crate::types::common::AuthMethod::Header => {
            let val = reqwest::header::HeaderValue::from_str(api_key).map_err(|_| {
                crate::error::BzrError::config("API key contains invalid header characters")
            })?;
            Ok(apply_auth_to_request(builder, Some(&val), None))
        }
        crate::types::common::AuthMethod::QueryParam => {
            Ok(apply_auth_to_request(builder, None, Some(api_key)))
        }
    }
}

/// Redact a Bugzilla API key value out of a string for safe display.
///
/// Looks for the literal `Bugzilla_api_key=` marker and replaces the value up
/// to the next `&`, `)`, or space with `[REDACTED]`. If the marker is absent
/// the input is returned unchanged.
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
#[path = "bugzilla_auth_tests.rs"]
mod tests;
