#![expect(clippy::unwrap_used)]

use std::collections::HashMap;

use super::*;
use crate::types::{OutputFormat, QueryKind, SavedQuery};

fn capture_detail(name: &str, query: &SavedQuery, format: OutputFormat) -> String {
    let mut buf = Vec::new();
    write_query_detail(name, query, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

/// Shared test helper — mirrors the `QueryView` in `write_query_detail` for JSON assertions.
#[derive(serde::Serialize)]
struct QueryView<'a> {
    name: &'a str,
    #[serde(flatten)]
    query: &'a SavedQuery,
}

fn make_url_query() -> SavedQuery {
    SavedQuery {
        kind: QueryKind::Url,
        source_url: Some(
            "https://bugzilla.example.com/buglist.cgi?product=Firefox&f1=qa_contact".into(),
        ),
        server: Some("example".into()),
        product: vec!["Firefox".into()],
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
        ],
        limit: Some(100),
        ..SavedQuery::default()
    }
}

fn make_list_query() -> SavedQuery {
    SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        status: vec!["NEW".into(), "ASSIGNED".into()],
        priority: vec!["P1".into()],
        limit: Some(25),
        ..Default::default()
    }
}

fn make_search_query() -> SavedQuery {
    SavedQuery {
        kind: QueryKind::Search,
        quicksearch: Some("crash in tab".into()),
        limit: Some(10),
        ..Default::default()
    }
}

#[test]
fn query_saved_json_serializes() {
    let json = serde_json::json!({"name": "test-q", "action": "saved"});
    let parsed: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&json).unwrap()).unwrap();
    assert_eq!(parsed["name"], "test-q");
    assert_eq!(parsed["action"], "saved");
}

#[test]
fn query_list_json_serializes() {
    let mut queries: HashMap<String, SavedQuery> = HashMap::new();
    queries.insert("firefox-new".into(), make_list_query());
    queries.insert("crashes".into(), make_search_query());
    let json = serde_json::to_string_pretty(&queries).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["firefox-new"].is_object());
    assert_eq!(parsed["firefox-new"]["kind"], "list");
    assert_eq!(parsed["crashes"]["kind"], "search");
}

#[test]
fn query_detail_json_with_flatten() {
    let query = make_list_query();
    let view = QueryView {
        name: "test-q",
        query: &query,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["name"], "test-q");
    assert_eq!(parsed["kind"], "list");
    assert_eq!(parsed["product"][0], "Firefox");
    assert_eq!(parsed["limit"], 25);
}

#[test]
fn query_saved_message_renders_table_text() {
    assert_eq!(
        query_saved_message("firefox-new", "Saved"),
        "Saved query 'firefox-new'"
    );
}

#[test]
fn query_summary_line_renders_list_query() {
    let line = query_summary_line("aaa", &make_list_query());
    assert!(line.starts_with("aaa (kind=list"));
    assert!(line.contains("product=Firefox"));
    assert!(line.contains("status=NEW,ASSIGNED"));
    assert!(line.contains("limit=25"));
}

#[test]
fn query_summary_line_renders_search_query() {
    let line = query_summary_line("zzz", &make_search_query());
    assert!(line.starts_with("zzz (kind=search"));
    assert!(line.contains("search=\"crash in tab\""));
    assert!(line.contains("limit=10"));
}

#[test]
fn write_query_detail_table_renders_fields() {
    let mut query = make_list_query();
    query.component = vec!["General".into()];
    query.assignee = vec!["dev@example.com".into()];
    query.fields = Some("id,summary".into());
    query.exclude_fields = Some("comments".into());

    let output = capture_detail("firefox-new", &query, OutputFormat::Table);

    assert!(output.contains("Name"));
    assert!(output.contains("firefox-new"));
    assert!(output.contains("Kind"));
    assert!(output.contains("list"));
    assert!(output.contains("Product"));
    assert!(output.contains("Firefox"));
    assert!(output.contains("Component"));
    assert!(output.contains("General"));
    assert!(output.contains("Fields"));
    assert!(output.contains("id,summary"));
    assert!(output.contains("Exclude"));
    assert!(output.contains("comments"));
}

#[test]
fn query_detail_json_includes_url_fields() {
    let query = make_url_query();
    let view = QueryView {
        name: "url-q",
        query: &query,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["kind"], "url");
    assert_eq!(
        parsed["source_url"],
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&f1=qa_contact"
    );
    assert_eq!(parsed["server"], "example");
    assert_eq!(parsed["raw_params"].as_array().unwrap().len(), 2);
}

#[test]
fn query_summary_line_renders_url_query() {
    let line = query_summary_line("url-q", &make_url_query());
    assert!(line.starts_with("url-q (kind=url"));
    assert!(line.contains("product=Firefox"));
    assert!(line.contains("2 raw params"));
}

#[test]
fn write_query_detail_table_renders_url_fields() {
    let query = make_url_query();
    let output = capture_detail("url-q", &query, OutputFormat::Table);

    assert!(output.contains("Source URL"));
    assert!(output.contains("bugzilla.example.com"));
    assert!(output.contains("Server"));
    assert!(output.contains("example"));
    assert!(output.contains("Raw params"));
    assert!(output.contains('2'));
}

#[test]
fn write_query_detail_table_shows_date_filters() {
    let mut query = make_list_query();
    query.creation_time = Some("2026-04-01T00:00:00Z".into());
    query.last_change_time = Some("2026-04-15T00:00:00Z".into());

    let output = capture_detail("recent", &query, OutputFormat::Table);

    assert!(output.contains("Created since"), "missing label: {output}");
    assert!(
        output.contains("2026-04-01T00:00:00Z"),
        "missing creation_time value"
    );
    assert!(output.contains("Changed since"), "missing label: {output}");
    assert!(
        output.contains("2026-04-15T00:00:00Z"),
        "missing last_change_time value"
    );
}

#[test]
fn query_detail_json_includes_date_filters() {
    let mut query = make_list_query();
    query.creation_time = Some("2026-04-01T00:00:00Z".into());
    let view = QueryView {
        name: "recent",
        query: &query,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["creation_time"], "2026-04-01T00:00:00Z");
}

#[test]
fn query_summary_line_renders_date_filters() {
    let mut query = make_list_query();
    query.creation_time = Some("2026-04-01T00:00:00Z".into());
    query.last_change_time = Some("2026-04-15T00:00:00Z".into());
    let line = query_summary_line("recent", &query);
    assert!(
        line.contains("created>=2026-04-01T00:00:00Z"),
        "summary missing created>=: {line}"
    );
    assert!(
        line.contains("changed>=2026-04-15T00:00:00Z"),
        "summary missing changed>=: {line}"
    );
}

#[test]
fn query_list_names_sort_before_render() {
    let mut queries: HashMap<String, SavedQuery> = HashMap::new();
    queries.insert("zzz".into(), make_list_query());
    queries.insert("aaa".into(), make_search_query());

    let mut names: Vec<&str> = queries.keys().map(String::as_str).collect();
    names.sort_unstable();
    let rendered: Vec<String> = names
        .into_iter()
        .map(|name| query_summary_line(name, &queries[name]))
        .collect();

    assert_eq!(
        rendered,
        vec![
            "aaa (kind=search, search=\"crash in tab\", limit=10)".to_string(),
            "zzz (kind=list, product=Firefox, status=NEW,ASSIGNED, limit=25)".to_string(),
        ]
    );
}

#[test]
fn write_query_detail_table_shows_158_field_filters() {
    let mut query = make_list_query();
    query.whiteboard = vec!["needs-review".into()];
    query.target_milestone = vec!["5.0".into()];
    query.version = vec!["9.4".into()];
    query.op_sys = vec!["Linux".into()];
    query.platform = vec!["x86_64".into()];
    query.resolution = vec!["FIXED".into()];
    query.qa_contact = vec!["qa@example.com".into()];
    query.url = vec!["github.com/foo".into()];

    let output = capture_detail("complete", &query, OutputFormat::Table);

    assert!(output.contains("Whiteboard"), "missing label: {output}");
    assert!(output.contains("needs-review"));
    assert!(output.contains("Target Milestone"));
    assert!(output.contains("5.0"));
    assert!(output.contains("Version"));
    assert!(output.contains("9.4"));
    assert!(output.contains("OS"));
    assert!(output.contains("Linux"));
    assert!(output.contains("Platform"));
    assert!(output.contains("x86_64"));
    assert!(output.contains("Resolution"));
    assert!(output.contains("FIXED"));
    assert!(output.contains("QA Contact"));
    assert!(output.contains("qa@example.com"));
    assert!(output.contains("URL"));
    assert!(output.contains("github.com/foo"));
}

#[test]
fn query_detail_json_includes_158_field_filters() {
    let mut query = make_list_query();
    query.whiteboard = vec!["needs-review".into()];
    query.url = vec!["github.com/foo".into()];
    let view = QueryView {
        name: "complete",
        query: &query,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["whiteboard"][0], "needs-review");
    assert_eq!(parsed["url"][0], "github.com/foo");
}

#[test]
fn query_summary_line_does_not_widen_for_158_fields() {
    // Regression: the brief summary view must not widen when only
    // the new field filters are set. The 8 fields belong in the
    // detail view, not the one-line summary.
    let mut query = make_list_query();
    // Wipe the fields that DO appear in the summary so the only
    // populated state is from #158 filters.
    query.product = vec![];
    query.status = vec![];
    query.limit = None;
    query.whiteboard = vec!["wip".into()];
    query.resolution = vec!["FIXED".into()];
    query.url = vec!["github.com".into()];

    let line = query_summary_line("test", &query);
    assert!(
        !line.contains("whiteboard"),
        "summary mentions whiteboard: {line}"
    );
    assert!(
        !line.contains("resolution"),
        "summary mentions resolution: {line}"
    );
    assert!(!line.contains("url"), "summary mentions url: {line}");
}
