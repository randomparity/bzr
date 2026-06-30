#![expect(clippy::unwrap_used)]

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
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
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
fn classify_param_kinds() {
    assert!(matches!(classify_param("columnlist"), ParamKind::Ignored));
    assert!(matches!(classify_param("known_name"), ParamKind::KnownName));
    assert!(matches!(
        classify_param("query_based_on"),
        ParamKind::QueryBasedOn
    ));
    assert!(matches!(classify_param("limit"), ParamKind::Limit));
    assert!(matches!(
        classify_param("include_fields"),
        ParamKind::IncludeFields
    ));
    assert!(matches!(classify_param("fields"), ParamKind::IncludeFields));
    assert!(matches!(
        classify_param("exclude_fields"),
        ParamKind::ExcludeFields
    ));
    assert!(matches!(classify_param("product"), ParamKind::Mapped(_)));
    assert!(matches!(
        classify_param("Bugzilla_api_key"),
        ParamKind::Credential
    ));
    assert!(matches!(classify_param("token"), ParamKind::Credential));
    assert!(matches!(
        classify_param("nonexistent_field"),
        ParamKind::Raw
    ));
}

/// Parse a buglist.cgi URL with the standard test config (bugzilla.example.com).
fn parse_test_url(query: &str) -> ParsedUrl {
    let config = make_config("https://bugzilla.example.com");
    let url = format!("https://bugzilla.example.com/buglist.cgi?{query}");
    parse_bugzilla_url(&url, &config).unwrap()
}

/// Extract `raw_params` keys as a vec for assertions.
fn raw_keys(parsed: &ParsedUrl) -> Vec<&str> {
    parsed
        .query
        .raw_params
        .iter()
        .map(|(k, _)| k.as_str())
        .collect()
}

/// Find a raw param value by key.
fn raw_value<'a>(parsed: &'a ParsedUrl, key: &str) -> &'a str {
    parsed
        .query
        .raw_params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap()
}

#[test]
fn parse_simple_url_with_recognized_params() {
    let parsed = parse_test_url("product=Firefox&product=Thunderbird&bug_status=NEW&limit=50");
    assert_eq!(parsed.query.product, vec!["Firefox", "Thunderbird"]);
    assert_eq!(parsed.query.status, vec!["NEW"]);
    assert_eq!(parsed.query.limit, Some(50));
    assert!(parsed.query.raw_params.is_empty());
    assert_eq!(parsed.query.server.as_deref(), Some("test"));
}

#[test]
fn parse_url_field_selection_is_typed_not_raw() {
    let parsed = parse_test_url(
        "product=Firefox&include_fields=id,summary,cf_release&exclude_fields=comments",
    );

    assert_eq!(
        parsed.query.fields.as_deref(),
        Some("id,summary,cf_release")
    );
    assert_eq!(parsed.query.exclude_fields.as_deref(), Some("comments"));
    assert!(parsed.query.raw_params.is_empty());
}

#[test]
fn limit_zero_accepted_as_no_limit() {
    // Bugzilla treats limit=0 as "no limit"; we pass it through verbatim.
    let parsed = parse_test_url("product=Firefox&limit=0");
    assert_eq!(parsed.query.limit, Some(0));
}

#[test]
fn empty_limit_treated_as_absent() {
    let parsed = parse_test_url("product=Firefox&limit=");
    assert_eq!(parsed.query.limit, None);
}

#[test]
fn non_numeric_limit_rejected() {
    let config = make_config("https://bugzilla.example.com");
    let err = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?limit=abc",
        &config,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        matches!(err, BzrError::InputValidation { message: ref m, .. } if m.contains("limit")),
        "expected limit validation error, got {err:?}"
    );
}

#[test]
fn overflow_limit_rejected() {
    let config = make_config("https://bugzilla.example.com");
    let err = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?limit=99999999999",
        &config,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn parse_complex_boolean_chart_url() {
    let parsed = parse_test_url(
        "known_name=My+Query\
        &query_format=advanced\
        &list_id=12345\
        &columnlist=bug_id%2Csummary\
        &chfield=%5BBug+creation%5D\
        &chfieldfrom=-7d\
        &classification=Client+Software\
        &f1=component\
        &o1=equals\
        &v1=PDF+Viewer",
    );

    // Ignored params must not appear in raw_params
    let keys = raw_keys(&parsed);
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
    assert_eq!(raw_value(&parsed, "v1"), "PDF Viewer");
    assert_eq!(raw_value(&parsed, "chfield"), "[Bug creation]");
}

#[test]
fn parse_url_without_buglist_cgi_errors() {
    let config = make_config("https://bugzilla.example.com");
    let err = parse_bugzilla_url(
        "https://bugzilla.example.com/show_bug.cgi?id=12345",
        &config,
    )
    .unwrap_err();
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
    let parsed = parse_test_url("product=Firefox&bug_status=NEW");
    assert_eq!(parsed.query.server.as_deref(), Some("test"));
}

#[test]
fn parse_url_hostname_no_match_uses_default() {
    let config = make_config("https://other.example.com");
    // Config has "test" server at other.example.com, with default_server = "test"
    // URL hostname (bugzilla.example.com) won't match
    let parsed = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox",
        &config,
    )
    .unwrap();
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
    let err = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox",
        &config,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("does not match"),
        "error should mention does not match: {err}"
    );
}

#[test]
fn parse_url_repeated_product_params_accumulate() {
    let parsed = parse_test_url("product=Firefox&product=Thunderbird&product=SeaMonkey");
    assert_eq!(
        parsed.query.product,
        vec!["Firefox", "Thunderbird", "SeaMonkey"]
    );
}

#[test]
fn parse_url_decodes_percent_encoded_values() {
    let parsed = parse_test_url("product=PPC64%20Development&assigned_to=user%40example.com");
    assert_eq!(parsed.query.product, vec!["PPC64 Development"]);
    assert_eq!(parsed.query.assignee, vec!["user@example.com"]);
}

#[test]
fn parse_url_all_recognized_fields() {
    let parsed = parse_test_url(
        "product=Firefox\
        &component=General\
        &bug_status=NEW\
        &assigned_to=dev@example.com\
        &reporter=reporter@example.com\
        &priority=P1\
        &bug_severity=major\
        &limit=100",
    );
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
    let parsed = parse_test_url("f1=component&o1=equals&v1=PDF+Viewer");
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
    let parsed = parse_test_url(
        "product=Firefox&Bugzilla_api_key=secret123&f1=component&o1=equals&v1=General",
    );

    // API key must not appear in raw_params
    let keys = raw_keys(&parsed);
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
    let parsed = parse_test_url("product=Firefox&Bugzilla_api_key=secret123&token=abc");

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
fn parses_known_name_into_suggested_name() {
    let parsed = parse_test_url("product=Firefox&known_name=my%20saved%20search");
    assert_eq!(parsed.suggested_name, Some("my saved search".into()));
}

#[test]
fn prefers_known_name_over_query_based_on() {
    let parsed = parse_test_url("product=Firefox&known_name=preferred&query_based_on=ancestor");
    assert_eq!(parsed.suggested_name, Some("preferred".into()));
}

#[test]
fn falls_back_to_query_based_on() {
    let parsed = parse_test_url("product=Firefox&query_based_on=ancestor%20query");
    assert_eq!(parsed.suggested_name, Some("ancestor query".into()));
}

#[test]
fn no_suggested_name_when_absent() {
    let parsed = parse_test_url("product=Firefox");
    assert!(parsed.suggested_name.is_none());
}

#[test]
fn empty_known_name_ignored() {
    let parsed = parse_test_url("product=Firefox&known_name=");
    assert!(parsed.suggested_name.is_none());
}

#[test]
fn parse_url_strips_credentials_case_insensitive() {
    let parsed = parse_test_url("product=Firefox&BUGZILLA_API_KEY=secret&Token=abc&api_key=def");

    let source = parsed.query.source_url.as_deref().unwrap();
    assert!(!source.contains("secret"));
    assert!(!source.contains("abc"));
    assert!(!source.contains("def"));

    let keys = raw_keys(&parsed);
    assert!(
        keys.is_empty()
            || !keys
                .iter()
                .any(|k| k.to_ascii_lowercase().contains("key") || k.eq_ignore_ascii_case("token"))
    );
}

#[test]
fn strip_shell_backslashes_cleans_escaped_url() {
    let escaped = r"https://bugzilla.example.com/buglist.cgi\?product\=Firefox\&bug_status\=NEW";
    let cleaned = strip_shell_backslashes(escaped);
    assert_eq!(
        cleaned,
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW"
    );
}

#[test]
fn strip_shell_backslashes_preserves_clean_url() {
    let clean = "https://bugzilla.example.com/buglist.cgi?product=Firefox";
    let result = strip_shell_backslashes(clean);
    assert_eq!(result, clean);
}

#[test]
fn strip_shell_backslashes_preserves_percent_encoding() {
    // Backslash before % should be stripped; the %20 must survive intact
    let escaped = r"https://bugzilla.example.com/buglist.cgi\?product\=PPC64\%20Development";
    let cleaned = strip_shell_backslashes(escaped);
    assert_eq!(
        cleaned,
        "https://bugzilla.example.com/buglist.cgi?product=PPC64%20Development"
    );
}

#[test]
fn strip_shell_backslashes_ignores_non_special() {
    // Backslash before a non-URL-significant character is kept
    let input = r"https://bugzilla.example.com/buglist.cgi?summary=foo\bar";
    let result = strip_shell_backslashes(input);
    assert_eq!(result, input);
}

#[test]
fn strip_shell_backslashes_trailing_backslash() {
    // Trailing backslash with no following char is preserved
    let input = r"https://bugzilla.example.com/buglist.cgi?product=Firefox\";
    let result = strip_shell_backslashes(input);
    assert_eq!(result, input);
}

#[test]
fn parse_url_with_shell_backslashes_succeeds() {
    let config = make_config("https://bugzilla.example.com");
    let escaped = r"https://bugzilla.example.com/buglist.cgi\?product\=Firefox\&bug_status\=NEW";
    let parsed = parse_bugzilla_url(escaped, &config).unwrap();
    assert_eq!(parsed.query.product, vec!["Firefox"]);
    assert_eq!(parsed.query.status, vec!["NEW"]);
}

#[test]
fn parse_url_with_shell_backslashes_boolean_chart() {
    let config = make_config("https://bugzilla.example.com");
    let escaped = r"https://bugzilla.example.com/buglist.cgi\?f1\=qa_contact\&o1\=changedfrom\&v1\=user\%40example.com\&classification\=Community";
    let parsed = parse_bugzilla_url(escaped, &config).unwrap();

    let keys = raw_keys(&parsed);
    assert!(keys.contains(&"f1"), "boolean chart f1 missing: {keys:?}");
    assert!(keys.contains(&"o1"), "boolean chart o1 missing: {keys:?}");
    assert_eq!(raw_value(&parsed, "v1"), "user@example.com");
    assert_eq!(raw_value(&parsed, "classification"), "Community");
}

#[test]
fn parse_url_with_new_158_field_params() {
    let parsed = parse_test_url(
        "status_whiteboard=needs-review\
        &target_milestone=5.0\
        &version=9.4\
        &op_sys=Linux\
        &rep_platform=x86_64\
        &resolution=FIXED\
        &qa_contact=qa%40example.com\
        &bug_file_loc=github.com%2Ffoo",
    );
    assert_eq!(parsed.query.whiteboard, vec!["needs-review"]);
    assert_eq!(parsed.query.target_milestone, vec!["5.0"]);
    assert_eq!(parsed.query.version, vec!["9.4"]);
    assert_eq!(parsed.query.op_sys, vec!["Linux"]);
    assert_eq!(parsed.query.platform, vec!["x86_64"]);
    assert_eq!(parsed.query.resolution, vec!["FIXED"]);
    assert_eq!(parsed.query.qa_contact, vec!["qa@example.com"]);
    assert_eq!(parsed.query.url, vec!["github.com/foo"]);
    // Nothing falls through to raw_params.
    assert!(parsed.query.raw_params.is_empty());
}

#[test]
fn parse_url_repeated_whiteboard_accumulates() {
    let parsed = parse_test_url("status_whiteboard=wip&status_whiteboard=review");
    assert_eq!(parsed.query.whiteboard, vec!["wip", "review"]);
}
