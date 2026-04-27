//! Parse Bugzilla `buglist.cgi` URLs into `SavedQuery` structs.

use url::Url;

use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::{QueryKind, SavedQuery};

/// Parameters ignored during URL parsing (display/session metadata).
const IGNORED_PARAMS: &[&str] = &["columnlist", "list_id", "query_format"];

/// Parameters extracted as the suggested query name, not stored as filters.
const NAME_PARAMS: &[&str] = &["known_name", "query_based_on"];

/// Maps Bugzilla URL parameter names to `SavedQuery` vec field names.
const RECOGNIZED_VEC_PARAMS: &[(&str, &str)] = &[
    ("product", "product"),
    ("component", "component"),
    ("bug_status", "status"),
    ("assigned_to", "assignee"),
    ("reporter", "creator"),
    ("priority", "priority"),
    ("bug_severity", "severity"),
];

/// Result of parsing a Bugzilla URL.
#[derive(Debug)]
pub struct ParsedUrl {
    pub query: SavedQuery,
    pub suggested_name: Option<String>,
}

/// Parse a Bugzilla `buglist.cgi` URL into a `SavedQuery`.
///
/// Recognized parameters are mapped to structured `SavedQuery` fields.
/// Unrecognized parameters are stored in `raw_params` for verbatim
/// passthrough to the REST API. Display/session params are ignored.
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
        source_url: Some(url_str.to_string()),
        server: server.map(String::from),
        ..SavedQuery::default()
    };

    let mut suggested_name: Option<String> = None;

    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        let value = value.as_ref();

        if IGNORED_PARAMS.contains(&key) {
            continue;
        }

        if NAME_PARAMS.contains(&key) {
            if suggested_name.is_none() && !value.is_empty() {
                suggested_name = Some(value.to_string());
            }
            continue;
        }

        if key == "limit" {
            if let Ok(n) = value.parse::<u32>() {
                query.limit = Some(n);
            }
            continue;
        }

        if let Some(&(_, field_name)) = RECOGNIZED_VEC_PARAMS
            .iter()
            .find(|&&(url_key, _)| url_key == key)
        {
            let target = match field_name {
                "product" => &mut query.product,
                "component" => &mut query.component,
                "status" => &mut query.status,
                "assignee" => &mut query.assignee,
                "creator" => &mut query.creator,
                "priority" => &mut query.priority,
                "severity" => &mut query.severity,
                _ => unreachable!(),
            };
            target.push(value.to_string());
            continue;
        }

        query.raw_params.push((key.to_string(), value.to_string()));
    }

    Ok(ParsedUrl {
        query,
        suggested_name,
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

        assert_eq!(parsed.suggested_name.as_deref(), Some("My Query"));

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
}
