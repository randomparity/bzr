use std::collections::HashMap;
use std::io::{self, Write as _};

use crate::types::{OutputFormat, QueryKind, SavedQuery};

use super::formatting::{
    print_field, print_formatted, print_json, print_list_field, print_optional_field,
};

fn kind_label(kind: &QueryKind) -> &'static str {
    match kind {
        QueryKind::List => "list",
        QueryKind::Search => "search",
        QueryKind::Url => "url",
    }
}

fn query_saved_message(name: &str, verb: &str) -> String {
    format!("{verb} query '{name}'")
}

fn query_summary_line(name: &str, q: &SavedQuery) -> String {
    let mut parts = vec![format!("kind={}", kind_label(&q.kind))];
    if !q.product.is_empty() {
        parts.push(format!("product={}", q.product.join(",")));
    }
    if !q.status.is_empty() {
        parts.push(format!("status={}", q.status.join(",")));
    }
    if let Some(qs) = &q.quicksearch {
        parts.push(format!("search=\"{qs}\""));
    }
    if let Some(ct) = &q.creation_time {
        parts.push(format!("created>={ct}"));
    }
    if let Some(lct) = &q.last_change_time {
        parts.push(format!("changed>={lct}"));
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
            let _ = writeln!(io::stdout(), "{}", query_saved_message(name, verb));
        }
    }
}

pub fn print_query_list(queries: &HashMap<String, SavedQuery>, format: OutputFormat) {
    print_formatted(queries, format, |queries| {
        if queries.is_empty() {
            let _ = writeln!(io::stdout(), "No saved queries configured.");
            return;
        }
        let mut names: Vec<&str> = queries.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let _ = writeln!(io::stdout(), "{}", query_summary_line(name, &queries[name]));
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
        print_field("Name", view.name);
        print_field("Kind", kind_label(&view.query.kind));
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
        print_optional_field("Created since", view.query.creation_time.as_deref());
        print_optional_field("Changed since", view.query.last_change_time.as_deref());
        print_list_field("Whiteboard", &view.query.whiteboard);
        print_list_field("Target Milestone", &view.query.target_milestone);
        print_list_field("Version", &view.query.version);
        print_list_field("OS", &view.query.op_sys);
        print_list_field("Platform", &view.query.platform);
        print_list_field("Resolution", &view.query.resolution);
        print_list_field("QA Contact", &view.query.qa_contact);
        print_list_field("URL", &view.query.url);
        if !view.query.raw_params.is_empty() {
            print_field("Raw params", &view.query.raw_params.len().to_string());
        }
    });
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
