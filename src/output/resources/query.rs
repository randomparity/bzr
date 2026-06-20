use std::collections::HashMap;
use std::io::Write;

use crate::types::{OutputFormat, QueryKind, SavedQuery};

use crate::output::formatting::{
    write_field, write_formatted, write_list_field, write_optional_field,
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

pub fn write_query_saved<W: Write + ?Sized>(
    name: &str,
    verb: &str,
    format: OutputFormat,
    out: &mut W,
) {
    crate::output::result_types::write_result(
        &serde_json::json!({"name": name, "action": verb.to_lowercase()}),
        &query_saved_message(name, verb),
        format,
        out,
    );
}

pub fn write_query_list<W: Write + ?Sized, S: ::std::hash::BuildHasher>(
    queries: &HashMap<String, SavedQuery, S>,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(queries, format, out, |queries, out| {
        if queries.is_empty() {
            let _ = writeln!(out, "No saved queries configured.");
            return;
        }
        let mut names: Vec<&str> = queries.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let _ = writeln!(out, "{}", query_summary_line(name, &queries[name]));
        }
    });
}

pub fn write_query_detail<W: Write + ?Sized>(
    name: &str,
    query: &SavedQuery,
    format: OutputFormat,
    out: &mut W,
) {
    #[derive(serde::Serialize)]
    struct QueryView<'a> {
        name: &'a str,
        #[serde(flatten)]
        query: &'a SavedQuery,
    }

    let view = QueryView { name, query };
    write_formatted(&view, format, out, |view, out| {
        write_field(out, "Name", view.name);
        write_field(out, "Kind", kind_label(&view.query.kind));
        write_optional_field(out, "Source URL", view.query.source_url.as_deref());
        write_optional_field(out, "Server", view.query.server.as_deref());
        write_list_field(out, "Product", &view.query.product);
        write_list_field(out, "Component", &view.query.component);
        write_list_field(out, "Status", &view.query.status);
        write_list_field(out, "Assignee", &view.query.assignee);
        write_list_field(out, "Creator", &view.query.creator);
        write_list_field(out, "Priority", &view.query.priority);
        write_list_field(out, "Severity", &view.query.severity);
        write_optional_field(out, "Search", view.query.quicksearch.as_deref());
        if let Some(limit) = view.query.limit {
            write_field(out, "Limit", &limit.to_string());
        }
        write_optional_field(out, "Fields", view.query.fields.as_deref());
        write_optional_field(out, "Exclude", view.query.exclude_fields.as_deref());
        write_optional_field(out, "Created since", view.query.creation_time.as_deref());
        write_optional_field(out, "Changed since", view.query.last_change_time.as_deref());
        write_list_field(out, "Whiteboard", &view.query.whiteboard);
        write_list_field(out, "Target Milestone", &view.query.target_milestone);
        write_list_field(out, "Version", &view.query.version);
        write_list_field(out, "OS", &view.query.op_sys);
        write_list_field(out, "Platform", &view.query.platform);
        write_list_field(out, "Resolution", &view.query.resolution);
        write_list_field(out, "QA Contact", &view.query.qa_contact);
        write_list_field(out, "URL", &view.query.url);
        if !view.query.raw_params.is_empty() {
            write_field(out, "Raw params", &view.query.raw_params.len().to_string());
        }
    });
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
