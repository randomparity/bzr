//! Parse Bugzilla `buglist.cgi` URLs into `SavedQuery` structs.

use url::Url;

use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::{QueryKind, SavedQuery};

/// Parameters containing credentials that must not be stored or forwarded.
const CREDENTIAL_PARAMS: &[&str] = &["bugzilla_api_key", "token", "api_key"];

/// Parameters ignored during URL parsing (display/session metadata).
const IGNORED_PARAMS: &[&str] = &[
    "columnlist",
    "list_id",
    "query_format",
    "known_name",
    "query_based_on",
];

/// Result of parsing a Bugzilla URL.
#[derive(Debug)]
pub struct ParsedUrl {
    pub query: SavedQuery,
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

/// Parse a Bugzilla `buglist.cgi` URL into a `SavedQuery`.
///
/// Recognized parameters are mapped to structured `SavedQuery` fields.
/// Unrecognized parameters are stored in `raw_params` for verbatim
/// passthrough to the REST API. Display/session params are ignored.
/// Credential parameters are stripped from both `source_url` and `raw_params`.
pub fn parse_bugzilla_url(url_str: &str, config: &Config) -> Result<ParsedUrl> {
    let url =
        Url::parse(url_str).map_err(|e| BzrError::InputValidation(format!("invalid URL: {e}")))?;

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

    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        let value = value.as_ref();

        if IGNORED_PARAMS.contains(&key) {
            continue;
        }

        if key == "limit" {
            if let Ok(n) = value.parse::<u32>() {
                query.limit = Some(n);
            }
            continue;
        }

        // Recognized vec fields — map Bugzilla URL param names to SavedQuery fields
        let target = match key {
            "product" => Some(&mut query.product),
            "component" => Some(&mut query.component),
            "bug_status" => Some(&mut query.status),
            "assigned_to" => Some(&mut query.assignee),
            "reporter" => Some(&mut query.creator),
            "priority" => Some(&mut query.priority),
            "bug_severity" => Some(&mut query.severity),
            _ => None,
        };
        if let Some(target) = target {
            target.push(value.to_string());
            continue;
        }

        // Strip credential params — never store or forward these
        if CREDENTIAL_PARAMS.contains(&key.to_ascii_lowercase().as_str()) {
            tracing::warn!("stripping credential parameter '{key}' from URL");
            continue;
        }

        query.raw_params.push((key.to_string(), value.to_string()));
    }

    Ok(ParsedUrl { query })
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
#[expect(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::{Config, ServerConfig};

    fn make_server_config(server_url: &str) -> ServerConfig {
        ServerConfig {
            url: server_url.into(),
            api_key: None,
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
        }
    }

    fn make_config(server_url: &str) -> Config {
        let mut servers = HashMap::new();
        servers.insert("test".to_string(), make_server_config(server_url));
        Config {
            default_server: Some("test".to_string()),
            servers,
            templates: HashMap::new(),
            queries: HashMap::new(),
        }
    }

    #[test]
    fn parse_simple_url_with_recognized_params() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?product=Firefox&product=Thunderbird&bug_status=NEW&limit=50";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.product, vec!["Firefox", "Thunderbird"]);
        assert_eq!(parsed.query.status, vec!["NEW"]);
        assert_eq!(parsed.query.limit, Some(50));
        assert!(parsed.query.raw_params.is_empty());
        assert_eq!(parsed.query.server.as_deref(), Some("test"));
    }

    #[test]
    fn parse_complex_boolean_chart_url() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?known_name=My+Query\
            &query_format=advanced\
            &list_id=12345\
            &columnlist=bug_id%2Csummary\
            &chfield=%5BBug+creation%5D\
            &chfieldfrom=-7d\
            &classification=Client+Software\
            &f1=component\
            &o1=equals\
            &v1=PDF+Viewer";

        let parsed = parse_bugzilla_url(url, &config).unwrap();

        // Ignored params must not appear in raw_params
        let keys: Vec<&str> = parsed
            .query
            .raw_params
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(!keys.contains(&"query_format"));
        assert!(!keys.contains(&"list_id"));
        assert!(!keys.contains(&"columnlist"));
        assert!(!keys.contains(&"known_name"));

        // Boolean chart params must be in raw_params
        assert!(keys.contains(&"f1"));
        assert!(keys.contains(&"o1"));
        assert!(keys.contains(&"v1"));
        assert!(keys.contains(&"chfield"));
        assert!(keys.contains(&"chfieldfrom"));
        assert!(keys.contains(&"classification"));

        // URL-decoded values
        let v1 = parsed
            .query
            .raw_params
            .iter()
            .find(|(k, _)| k == "v1")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert_eq!(v1, "PDF Viewer");

        let chfield = parsed
            .query
            .raw_params
            .iter()
            .find(|(k, _)| k == "chfield")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert_eq!(chfield, "[Bug creation]");
    }

    #[test]
    fn parse_url_without_buglist_cgi_errors() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/show_bug.cgi?id=12345";

        let err = parse_bugzilla_url(url, &config).unwrap_err();
        assert!(
            err.to_string().contains("buglist.cgi"),
            "error should mention buglist.cgi: {err}"
        );
    }

    #[test]
    fn parse_malformed_url_errors() {
        let config = make_config("https://bugzilla.example.com");

        let err = parse_bugzilla_url("not a url", &config).unwrap_err();
        assert!(
            err.to_string().contains("invalid URL"),
            "error should mention invalid URL: {err}"
        );
    }

    #[test]
    fn parse_url_hostname_matches_configured_server() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.server.as_deref(), Some("test"));
    }

    #[test]
    fn parse_url_hostname_no_match_uses_default() {
        let config = make_config("https://other.example.com");
        // Config has "test" server at other.example.com, with default_server = "test"
        // URL hostname (bugzilla.example.com) won't match
        let url = "https://bugzilla.example.com/buglist.cgi?product=Firefox";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        // No hostname match → server field is None (will use default at query time)
        assert!(parsed.query.server.is_none());
    }

    #[test]
    fn parse_url_hostname_no_match_no_default_errors() {
        let config = Config {
            default_server: None,
            servers: HashMap::new(),
            templates: HashMap::new(),
            queries: HashMap::new(),
        };
        let url = "https://bugzilla.example.com/buglist.cgi?product=Firefox";

        let err = parse_bugzilla_url(url, &config).unwrap_err();
        assert!(
            err.to_string().contains("does not match"),
            "error should mention does not match: {err}"
        );
    }

    #[test]
    fn parse_url_repeated_product_params_accumulate() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?product=Firefox&product=Thunderbird&product=SeaMonkey";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(
            parsed.query.product,
            vec!["Firefox", "Thunderbird", "SeaMonkey"]
        );
    }

    #[test]
    fn parse_url_decodes_percent_encoded_values() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?product=PPC64%20Development\
            &assigned_to=user%40example.com";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.product, vec!["PPC64 Development"]);
        assert_eq!(parsed.query.assignee, vec!["user@example.com"]);
    }

    #[test]
    fn parse_url_all_recognized_fields() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?product=Firefox\
            &component=General\
            &bug_status=NEW\
            &assigned_to=dev@example.com\
            &reporter=reporter@example.com\
            &priority=P1\
            &bug_severity=major\
            &limit=100";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.product, vec!["Firefox"]);
        assert_eq!(parsed.query.component, vec!["General"]);
        assert_eq!(parsed.query.status, vec!["NEW"]);
        assert_eq!(parsed.query.assignee, vec!["dev@example.com"]);
        assert_eq!(parsed.query.creator, vec!["reporter@example.com"]);
        assert_eq!(parsed.query.priority, vec!["P1"]);
        assert_eq!(parsed.query.severity, vec!["major"]);
        assert_eq!(parsed.query.limit, Some(100));
        assert!(parsed.query.raw_params.is_empty());
    }

    #[test]
    fn parse_url_only_raw_params() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi\
            ?f1=component&o1=equals&v1=PDF+Viewer";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert!(parsed.query.product.is_empty());
        assert_eq!(parsed.query.raw_params.len(), 3);
        assert!(parsed.query.has_filters());
    }

    #[test]
    fn find_server_by_hostname_matches() {
        let config = make_config("https://bugzilla.example.com");
        let result = find_server_by_hostname(&config, "bugzilla.example.com");
        assert_eq!(result, Some("test"));
    }

    #[test]
    fn find_server_by_hostname_no_match() {
        let config = make_config("https://bugzilla.example.com");
        let result = find_server_by_hostname(&config, "other.example.com");
        assert!(result.is_none());
    }

    #[test]
    fn parse_url_strips_api_key_from_raw_params() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&Bugzilla_api_key=secret123&f1=component&o1=equals&v1=General";
        let parsed = parse_bugzilla_url(url, &config).unwrap();

        // API key must not appear in raw_params
        let keys: Vec<&str> = parsed
            .query
            .raw_params
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(!keys.contains(&"Bugzilla_api_key"));
        assert!(!keys.contains(&"bugzilla_api_key"));

        // Other raw params should still be present
        assert!(keys.contains(&"f1"));
        assert!(keys.contains(&"o1"));
        assert!(keys.contains(&"v1"));

        // Product should be recognized normally
        assert_eq!(parsed.query.product, vec!["Firefox"]);
    }

    #[test]
    fn parse_url_strips_credentials_from_source_url() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&Bugzilla_api_key=secret123&token=abc";
        let parsed = parse_bugzilla_url(url, &config).unwrap();

        let source = parsed.query.source_url.as_deref().unwrap();
        assert!(
            !source.contains("secret123"),
            "API key leaked into source_url: {source}"
        );
        assert!(
            !source.contains("abc"),
            "token leaked into source_url: {source}"
        );
        assert!(
            source.contains("product=Firefox"),
            "non-credential params should remain: {source}"
        );
    }

    #[test]
    fn parse_url_strips_credentials_case_insensitive() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&BUGZILLA_API_KEY=secret&Token=abc&api_key=def";
        let parsed = parse_bugzilla_url(url, &config).unwrap();

        let source = parsed.query.source_url.as_deref().unwrap();
        assert!(!source.contains("secret"));
        assert!(!source.contains("abc"));
        assert!(!source.contains("def"));

        let keys: Vec<&str> = parsed
            .query
            .raw_params
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(
            keys.is_empty()
                || !keys
                    .iter()
                    .any(|k| k.to_ascii_lowercase().contains("key")
                        || k.eq_ignore_ascii_case("token"))
        );
    }
}
