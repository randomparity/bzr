use std::cell::RefCell;

/// Bugzilla's non-standard auth header (not `Authorization`).
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Bugzilla's query-param auth key, used by servers that reject header auth.
pub(crate) const AUTH_QUERY_PARAM: &str = "Bugzilla_api_key";

const MIN_BARE_KEY_LEN: usize = 8;

thread_local! {
    static ACTIVE_API_KEY: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn clear_active_api_key() {
    ACTIVE_API_KEY.with(|slot| *slot.borrow_mut() = None);
}

pub(crate) fn register_active_api_key(api_key: &str) {
    if !api_key.is_empty() {
        ACTIVE_API_KEY.with(|slot| *slot.borrow_mut() = Some(api_key.to_string()));
    }
}

#[cfg(test)]
pub(crate) struct ActiveApiKeyTestGuard {
    previous: Option<String>,
}

#[cfg(test)]
impl Drop for ActiveApiKeyTestGuard {
    fn drop(&mut self) {
        ACTIVE_API_KEY.with(|slot| *slot.borrow_mut() = self.previous.take());
    }
}

#[cfg(test)]
pub(crate) fn active_api_key_test_guard(api_key: Option<&str>) -> ActiveApiKeyTestGuard {
    let previous = ACTIVE_API_KEY.with(|slot| slot.replace(api_key.map(String::from)));
    ActiveApiKeyTestGuard { previous }
}

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
    method: crate::types::transport::AuthMethod,
) -> crate::error::Result<reqwest::RequestBuilder> {
    match method {
        crate::types::transport::AuthMethod::Header => {
            let val = reqwest::header::HeaderValue::from_str(api_key).map_err(|_| {
                crate::error::BzrError::config("API key contains invalid header characters")
            })?;
            Ok(apply_auth_to_request(builder, Some(&val), None))
        }
        crate::types::transport::AuthMethod::QueryParam => {
            Ok(apply_auth_to_request(builder, None, Some(api_key)))
        }
    }
}

/// Ends an API key value in free text: any whitespace, or a URL/markup
/// delimiter.
///
/// None of these can truncate a redaction early, whatever charset a server's
/// key generator uses: [`apply_auth_to_request`] passes the key through
/// reqwest's `.query()`, which percent-encodes it, so the URL a server can echo
/// back never contains one of these characters unescaped.
///
/// Whitespace is a terminator because a raw HTTP error body is multi-line —
/// scanning past a newline would swallow the start of the following line and,
/// in the same motion, run over a second occurrence of the marker and leave
/// that key exposed.
fn ends_api_key_value(c: char) -> bool {
    c.is_whitespace() || matches!(c, '&' | ')' | '"' | '\'' | '<' | '>' | '#')
}

/// Redact every Bugzilla API key value out of a string for safe display.
///
/// Replaces the value following each literal `Bugzilla_api_key=` marker —
/// up to the next whitespace or `&`, `)`, `"`, `'`, `<`, `>`, `#` — with
/// `[REDACTED]`. A string with no marker is returned byte-for-byte unchanged,
/// and re-running the function over its own output is a no-op.
///
/// Every occurrence is redacted, not just the first: this runs over raw HTTP
/// error bodies (`BzrError::HttpStatus`), and an error page from a proxy or
/// `CGI::Carp` typically echoes the request URI more than once.
fn redact_marked_api_key(msg: &str, marker: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(idx) = rest.find(marker) {
        out.push_str(&rest[..idx + marker.len()]);
        out.push_str("[REDACTED]");
        let value = &rest[idx + marker.len()..];
        let end = value.find(ends_api_key_value).unwrap_or(value.len());
        rest = &value[end..];
    }
    out.push_str(rest);
    out
}

pub(crate) fn redact_api_key(msg: &str) -> String {
    let mut redacted = msg.to_string();
    for suffix in ["=", "%3D", "%3d"] {
        let marker = format!("{AUTH_QUERY_PARAM}{suffix}");
        redacted = redact_marked_api_key(&redacted, &marker);
    }
    ACTIVE_API_KEY.with(|slot| {
        let key = slot.borrow();
        match key.as_deref() {
            Some(key) if key.len() >= MIN_BARE_KEY_LEN => redacted.replace(key, "[REDACTED]"),
            _ => redacted,
        }
    })
}

#[cfg(test)]
#[path = "bugzilla_auth_tests.rs"]
mod tests;
