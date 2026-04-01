use std::collections::HashMap;

use crate::types::{OutputFormat, QueryKind, SavedQuery};

use super::formatting::{
    print_field, print_formatted, print_json, print_list_field, print_optional_field,
};

#[expect(dead_code, reason = "used by commands/query.rs once implemented")]
pub fn print_query_saved(name: &str, verb: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            print_json(&serde_json::json!({"name": name, "action": verb.to_lowercase()}));
        }
        OutputFormat::Table => {
            println!("{verb} query '{name}'");
        }
    }
}

#[expect(dead_code, reason = "used by commands/query.rs once implemented")]
pub fn print_query_list(queries: &HashMap<String, SavedQuery>, format: OutputFormat) {
    print_formatted(queries, format, |queries| {
        if queries.is_empty() {
            println!("No saved queries configured.");
            return;
        }
        let mut names: Vec<&str> = queries.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let q = &queries[name];
            let kind_label = match q.kind {
                QueryKind::List => "list",
                QueryKind::Search => "search",
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
            println!("{name} ({})", parts.join(", "));
        }
    });
}

#[expect(dead_code, reason = "used by commands/query.rs once implemented")]
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
        };
        print_field("Name", view.name);
        print_field("Kind", kind_label);
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
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
