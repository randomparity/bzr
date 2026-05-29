//! Parse Bugzilla `buglist.cgi` URLs into `SavedQuery` structs.

use url::Url;

use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::{QueryKind, SavedQuery, FIELD_MAPPINGS};

/// Parameters containing credentials that must not be stored or forwarded.
const CREDENTIAL_PARAMS: &[&str] = &["bugzilla_api_key", "token", "api_key"];

/// Parameters ignored during URL parsing (display/session metadata).
const IGNORED_PARAMS: &[&str] = &["columnlist", "list_id", "query_format"];

/// Classification of a URL query-pair key.
enum ParamKind {
    Ignored,
    KnownName,
    QueryBasedOn,
    Limit,
    Mapped(&'static crate::types::FieldMapping),
    Credential,
    Raw,
}

/// Classify a URL query-pair key into a `ParamKind`. Pure dispatch — no I/O,
/// no allocation other than ASCII lowercasing for credential matching.
fn classify_param(key: &str) -> ParamKind {
    if IGNORED_PARAMS.contains(&key) {
        return ParamKind::Ignored;
    }
    match key {
        "known_name" => return ParamKind::KnownName,
        "query_based_on" => return ParamKind::QueryBasedOn,
        "limit" => return ParamKind::Limit,
        _ => {}
    }
    if let Some(mapping) = FIELD_MAPPINGS.iter().find(|m| m.url_param == key) {
        return ParamKind::Mapped(mapping);
    }
    if CREDENTIAL_PARAMS.contains(&key.to_ascii_lowercase().as_str()) {
        return ParamKind::Credential;
    }
    ParamKind::Raw
}

/// Result of parsing a Bugzilla URL.
#[derive(Debug)]
#[non_exhaustive]
pub struct ParsedUrl {
    pub query: SavedQuery,
    /// Suggested name extracted from URL's `known_name` or `query_based_on` param.
    pub suggested_name: Option<String>,
}

/// Strip credential query parameters from a URL, returning the sanitized string.
fn sanitize_url(url: &Url) -> String {
    let mut sanitized = url.clone();
    let pairs: Vec<(String, String)> = sanitized
        .query_pairs()
        .filter(|(k, _)| !CREDENTIAL_PARAMS.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if pairs.is_empty() {
        sanitized.set_query(None);
    } else {
        sanitized.query_pairs_mut().clear().extend_pairs(pairs);
    }
    sanitized.to_string()
}

/// Strip backslashes that precede URL-significant characters (`?`, `&`, `=`).
///
/// When a user pastes a raw URL into zsh without quotes, the shell
/// auto-escapes these characters (e.g. `buglist.cgi\?foo\=bar\&baz`).
/// If the escaped form is then quoted or copied, the backslashes become
/// literal and pollute every query-param name and value (`chfield%5C`).
/// Backslashes before these characters are never valid in HTTP URLs, so
/// stripping them is always safe.
fn strip_shell_backslashes(url: &str) -> String {
    if !url.contains('\\') {
        return url.to_string();
    }
    tracing::warn!(
        "URL contains backslash-escaped characters (e.g. \\? \\& \\=); \
         stripping shell escapes — quote the URL to avoid this"
    );
    let mut out = String::with_capacity(url.len());
    let mut chars = url.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && matches!(chars.peek(), Some('?' | '&' | '=' | '%')) {
            continue;
        }
        out.push(ch);
    }
    out
}

/// Parse a Bugzilla `buglist.cgi` URL into a `SavedQuery`.
///
/// Recognized parameters are mapped to structured `SavedQuery` fields.
/// Unrecognized parameters are stored in `raw_params` for verbatim
/// passthrough to the REST API. Display/session params are ignored.
/// Credential parameters are stripped from both `source_url` and `raw_params`.
pub fn parse_bugzilla_url(url_str: &str, config: &Config) -> Result<ParsedUrl> {
    let cleaned = strip_shell_backslashes(url_str);
    let url =
        Url::parse(&cleaned).map_err(|e| BzrError::InputValidation(format!("invalid URL: {e}")))?;

    if !url.path().contains("buglist.cgi") {
        return Err(BzrError::InputValidation(
            "URL must be a Bugzilla buglist.cgi URL".into(),
        ));
    }

    let url_host = url
        .host_str()
        .ok_or_else(|| BzrError::InputValidation("URL has no hostname".into()))?;

    let server = find_server_by_hostname(config, url_host);
    if server.is_none() && config.default_server.is_none() {
        return Err(BzrError::config(format!(
            "URL hostname '{url_host}' does not match any configured server \
             and no default server is set. Run `bzr config set-server` first."
        )));
    }
    if server.is_none() {
        tracing::warn!(
            "URL hostname '{url_host}' does not match any configured server; \
             using default server"
        );
    }

    let mut query = SavedQuery {
        kind: QueryKind::Url,
        source_url: Some(sanitize_url(&url)),
        server: server.map(String::from),
        ..SavedQuery::default()
    };

    let mut known_name: Option<String> = None;
    let mut query_based_on: Option<String> = None;

    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        let value = value.as_ref();

        match classify_param(key) {
            ParamKind::Ignored => {}
            ParamKind::KnownName => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    known_name = Some(trimmed.to_string());
                }
            }
            ParamKind::QueryBasedOn => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    query_based_on = Some(trimmed.to_string());
                }
            }
            ParamKind::Limit => {
                let trimmed = value.trim();
                // An empty `limit=` carries no value; treat it as absent
                // (consistent with the other trimmed params above).
                if !trimmed.is_empty() {
                    // `limit=0` is accepted verbatim: Bugzilla interprets 0 as
                    // "no limit" (return all matches). Non-numeric or
                    // out-of-range values (e.g. `abc`, `99999999999`) are
                    // rejected here rather than silently dropped.
                    let n = trimmed.parse::<u32>().map_err(|_| {
                        BzrError::InputValidation(format!(
                            "URL limit '{trimmed}' is not a valid integer in 0..={}",
                            u32::MAX
                        ))
                    })?;
                    query.limit = Some(n);
                }
            }
            ParamKind::Mapped(mapping) => {
                let Some(target) = query.get_field_mut(mapping.struct_field) else {
                    unreachable!(
                        "FIELD_MAPPINGS struct_field '{}' missing from get_field_mut",
                        mapping.struct_field
                    );
                };
                target.push(value.to_string());
            }
            ParamKind::Credential => {
                tracing::warn!("stripping credential parameter '{key}' from URL");
            }
            ParamKind::Raw => {
                query.raw_params.push((key.to_string(), value.to_string()));
            }
        }
    }

    Ok(ParsedUrl {
        query,
        suggested_name: known_name.or(query_based_on),
    })
}

fn find_server_by_hostname<'a>(config: &'a Config, hostname: &str) -> Option<&'a str> {
    for (name, srv) in &config.servers {
        if let Ok(srv_url) = Url::parse(&srv.url) {
            if srv_url.host_str() == Some(hostname) {
                return Some(name.as_str());
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "url_parser_tests.rs"]
mod tests;
