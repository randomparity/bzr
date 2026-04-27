use std::collections::HashMap;

use crate::types::{OutputFormat, QueryKind, SavedQuery};

use super::formatting::{
    print_field, print_formatted, print_json, print_list_field, print_optional_field,
};

fn query_saved_message(name: &str, verb: &str) -> String {
    format!("{verb} query '{name}'")
}

fn query_summary_line(name: &str, q: &SavedQuery) -> String {
    let kind_label = match q.kind {
        QueryKind::List => "list",
        QueryKind::Search => "search",
        QueryKind::Url => "url",
    };
    let mut parts = vec![format!("kind={kind_label}")];
    if !q.product.is_empty() {
        parts.push(format!("product={}", q.product.join(",")));
    }
    if !q.status.is_empty() {
        parts.push(format!("status={}", q.status.join(",")));
    }
    if let Some(qs) = &q.quicksearch {
        parts.push(format!("search=\"{qs}\""));
    }
    if let Some(limit) = q.limit {
        parts.push(format!("limit={limit}"));
    }
    if !q.raw_params.is_empty() {
        parts.push(format!("{} raw params", q.raw_params.len()));
    }
    format!("{name} ({})", parts.join(", "))
}

pub fn print_query_saved(name: &str, verb: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            print_json(&serde_json::json!({"name": name, "action": verb.to_lowercase()}));
        }
        OutputFormat::Table => {
            println!("{}", query_saved_message(name, verb));
        }
    }
}

pub fn print_query_list(queries: &HashMap<String, SavedQuery>, format: OutputFormat) {
    print_formatted(queries, format, |queries| {
        if queries.is_empty() {
            println!("No saved queries configured.");
            return;
        }
        let mut names: Vec<&str> = queries.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            println!("{}", query_summary_line(name, &queries[name]));
        }
    });
}

pub fn print_query_detail(name: &str, query: &SavedQuery, format: OutputFormat) {
    #[derive(serde::Serialize)]
    struct QueryView<'a> {
        name: &'a str,
        #[serde(flatten)]
        query: &'a SavedQuery,
    }

    let view = QueryView { name, query };
    print_formatted(&view, format, |view| {
        let kind_label = match view.query.kind {
            QueryKind::List => "list",
            QueryKind::Search => "search",
            QueryKind::Url => "url",
        };
        print_field("Name", view.name);
        print_field("Kind", kind_label);
        print_optional_field("Source URL", view.query.source_url.as_deref());
        print_optional_field("Server", view.query.server.as_deref());
        print_list_field("Product", &view.query.product);
        print_list_field("Component", &view.query.component);
        print_list_field("Status", &view.query.status);
        print_list_field("Assignee", &view.query.assignee);
        print_list_field("Creator", &view.query.creator);
        print_list_field("Priority", &view.query.priority);
        print_list_field("Severity", &view.query.severity);
        print_optional_field("Search", view.query.quicksearch.as_deref());
        if let Some(limit) = view.query.limit {
            print_field("Limit", &limit.to_string());
        }
        print_optional_field("Fields", view.query.fields.as_deref());
        print_optional_field("Exclude", view.query.exclude_fields.as_deref());
        if !view.query.raw_params.is_empty() {
            print_field("Raw params", &view.query.raw_params.len().to_string());
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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
        #[derive(serde::Serialize)]
        struct QueryView<'a> {
            name: &'a str,
            #[serde(flatten)]
            query: &'a SavedQuery,
        }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn print_query_detail_table_renders_fields() {
        let _lock = crate::ENV_LOCK.lock().await;
        let mut query = make_list_query();
        query.component = vec!["General".into()];
        query.assignee = vec!["dev@example.com".into()];
        query.fields = Some("id,summary".into());
        query.exclude_fields = Some("comments".into());

        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_query_detail("firefox-new", &query, OutputFormat::Table);
        })
        .await;

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
        #[derive(serde::Serialize)]
        struct QueryView<'a> {
            name: &'a str,
            #[serde(flatten)]
            query: &'a SavedQuery,
        }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn print_query_detail_table_renders_url_fields() {
        let _lock = crate::ENV_LOCK.lock().await;
        let query = make_url_query();

        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_query_detail("url-q", &query, OutputFormat::Table);
        })
        .await;

        assert!(output.contains("Source URL"));
        assert!(output.contains("bugzilla.example.com"));
        assert!(output.contains("Server"));
        assert!(output.contains("example"));
        assert!(output.contains("Raw params"));
        assert!(output.contains('2'));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn query_list_names_sort_before_render() {
        let _lock = crate::ENV_LOCK.lock().await;
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
}
