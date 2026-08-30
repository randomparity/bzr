use std::collections::HashSet;
use std::io::Write;

use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;

use crate::output::formatting::{
    colorize_status, disable_color_for_tests, render_flags_inline, shorten_email, truncate,
    write_divider, write_field, write_formatted, write_json_family, SUMMARY_TRUNCATE_WIDTH,
};
use crate::types::bug::{
    apply_exclude, canonical_excludes, canonical_field_list, default_selected_fields,
    field_selected, partition_include, selected_custom_detail_fields, Bug, BugAdjacencyError,
    BugAdjacencyRequest, BugAdjacencyResult, BugField, BugLink, ColumnSpec, HistoryEntry,
    HistoryRecord, SelectedBugField,
};
use crate::types::output::OutputFormat;

/// Bugzilla's sentinel for "no target milestone set". Suppressed in detail
/// output so a bug without a milestone does not print a noise row.
const UNSET_MILESTONE: &str = "---";

fn join_ids(ids: &[u64]) -> String {
    ids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve `spec` into the ordered list of columns to render. Unknown
/// include tokens are reported as a warning on `err`. If every requested
/// token is unknown, falls back to the default column set so output stays
/// useful. Infallible by design: callers reject fully-degenerate selections
/// before the network request.
fn resolve_columns<'a, E: Write + ?Sized>(
    spec: ColumnSpec<'a>,
    err: &mut E,
) -> Vec<SelectedBugField<'a>> {
    let mut columns = match spec.include {
        None => default_selected_fields(),
        Some(list) => {
            let partition = partition_include(list);
            if !partition.unknown.is_empty() {
                let _ = writeln!(
                    err,
                    "warning: ignoring unknown field(s): {}",
                    partition.unknown.join(", ")
                );
            }
            if partition.ordered.is_empty() {
                default_selected_fields()
            } else {
                partition.ordered
            }
        }
    };
    apply_exclude(&mut columns, spec.exclude);
    columns
}

/// Project a serialized bug object to honor `spec` (gh-style trimming):
/// a non-blank `include` retains exactly the named canonical keys; `exclude`
/// drops the named canonical keys; neither (or a blank include) leaves the
/// object untouched. Aliases resolve to canonical keys via the same primitives
/// table mode uses. Unknown tokens are inert — they name no key in the object,
/// so they neither add nor remove anything; command preflight owns unknown
/// field warnings.
pub fn bug_to_json(bug: &Bug, spec: ColumnSpec<'_>) -> serde_json::Value {
    let mut value = serde_json::to_value(bug).expect("Bug serializes to JSON");
    if let serde_json::Value::Object(map) = &mut value {
        if let Some(include) = canonical_field_list(spec.include) {
            let keep: HashSet<&str> = include.split(',').collect();
            map.retain(|k, _| keep.contains(k.as_str()));
        }
        for canonical in canonical_excludes(spec.exclude) {
            map.remove(canonical);
        }
    }
    value
}

/// [`bug_to_json`] over a slice, for the array output paths.
pub fn bugs_to_json(bugs: &[Bug], spec: ColumnSpec<'_>) -> Vec<serde_json::Value> {
    bugs.iter().map(|bug| bug_to_json(bug, spec)).collect()
}

fn render_custom_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => value.to_string(),
        Some(Value::Null) | None => String::new(),
    }
}

fn render_selected_field(field: SelectedBugField<'_>, bug: &Bug) -> String {
    match field {
        SelectedBugField::BuiltIn(field) => render_builtin_field(field, bug),
        SelectedBugField::Custom(name) => render_custom_value(bug.custom_fields.get(name)),
    }
}

fn render_builtin_field(field: BugField, bug: &Bug) -> String {
    match field {
        BugField::Id => bug.id.to_string(),
        BugField::Status => bug.status.clone().unwrap_or_default(),
        BugField::Priority => bug.priority.clone().unwrap_or_default(),
        BugField::AssignedTo => shorten_email(bug.assigned_to.as_deref().unwrap_or("")),
        BugField::Summary => truncate(bug.summary.as_deref().unwrap_or(""), SUMMARY_TRUNCATE_WIDTH),
        BugField::Severity => bug.severity.clone().unwrap_or_default(),
        BugField::Product => bug.product.clone().unwrap_or_default(),
        BugField::Component => bug.component.clone().unwrap_or_default(),
        BugField::Resolution => bug.resolution.clone().unwrap_or_default(),
        BugField::Version => bug.version.clone().unwrap_or_default(),
        BugField::Creator => bug.creator.clone().unwrap_or_default(),
        BugField::CreationTime => bug.creation_time.clone().unwrap_or_default(),
        BugField::LastChangeTime => bug.last_change_time.clone().unwrap_or_default(),
        BugField::Url => bug.url.clone().unwrap_or_default(),
        BugField::Whiteboard => bug.whiteboard.clone().unwrap_or_default(),
        BugField::OpSys => bug.op_sys.clone().unwrap_or_default(),
        BugField::RepPlatform => bug.rep_platform.clone().unwrap_or_default(),
        BugField::Deadline => bug.deadline.clone().unwrap_or_default(),
        BugField::Keywords => bug.keywords.join(", "),
        BugField::Blocks => join_ids(&bug.blocks),
        BugField::DependsOn => join_ids(&bug.depends_on),
        BugField::Cc => bug.cc.join(", "),
        BugField::DupeOf => bug.dupe_of.map(|id| id.to_string()).unwrap_or_default(),
        BugField::TargetMilestone => bug.target_milestone.clone().unwrap_or_default(),
        BugField::Flags => render_flags_inline(&bug.flags),
    }
}

pub fn write_bugs<W: Write + ?Sized, E: Write + ?Sized>(
    bugs: &[Bug],
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    out: &mut W,
    err: &mut E,
) {
    disable_color_for_tests();
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_json_family(&bugs_to_json(bugs, spec), format, out);
        }
        OutputFormat::Table => {
            if bugs.is_empty() {
                let _ = writeln!(out, "No bugs found.");
                return;
            }
            let columns = resolve_columns(spec, err);
            let mut builder = Builder::default();
            builder.push_record(columns.iter().map(|field| (*field).header()));
            for bug in bugs {
                builder.push_record(
                    columns
                        .iter()
                        .map(|field| render_selected_field(*field, bug)),
                );
            }
            let _ = writeln!(out, "{}", builder.build());
        }
    }
}

pub fn write_bug_detail<W: Write + ?Sized>(
    bug: &Bug,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    out: &mut W,
) {
    disable_color_for_tests();
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_json_family(&bug_to_json(bug, spec), format, out);
        }
        OutputFormat::Table => write_bug_detail_table(bug, spec, out),
    }
}

struct DetailRow {
    label: &'static str,
    value: String,
}

#[derive(Clone, Copy)]
enum DetailValue {
    Status,
    OptionalText(fn(&Bug) -> Option<&str>),
    OptionalId(fn(&Bug) -> Option<u64>),
    StringList(fn(&Bug) -> &[String]),
    IdList(fn(&Bug) -> &[u64]),
    TargetMilestone,
    Flags,
}

#[derive(Clone, Copy)]
struct BuiltinDetailField {
    field: BugField,
    label: &'static str,
    value: DetailValue,
}

const BUILTIN_DETAIL_FIELDS: &[BuiltinDetailField] = &[
    BuiltinDetailField {
        field: BugField::Status,
        label: "Status",
        value: DetailValue::Status,
    },
    BuiltinDetailField {
        field: BugField::Resolution,
        label: "Resolution",
        value: DetailValue::OptionalText(|bug| bug.resolution.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::DupeOf,
        label: "Duplicate of",
        value: DetailValue::OptionalId(|bug| bug.dupe_of),
    },
    BuiltinDetailField {
        field: BugField::Product,
        label: "Product",
        value: DetailValue::OptionalText(|bug| bug.product.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::Component,
        label: "Component",
        value: DetailValue::OptionalText(|bug| bug.component.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::TargetMilestone,
        label: "Target Milestone",
        value: DetailValue::TargetMilestone,
    },
    BuiltinDetailField {
        field: BugField::AssignedTo,
        label: "Assignee",
        value: DetailValue::OptionalText(|bug| bug.assigned_to.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::Priority,
        label: "Priority",
        value: DetailValue::OptionalText(|bug| bug.priority.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::Severity,
        label: "Severity",
        value: DetailValue::OptionalText(|bug| bug.severity.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::Creator,
        label: "Creator",
        value: DetailValue::OptionalText(|bug| bug.creator.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::CreationTime,
        label: "Created",
        value: DetailValue::OptionalText(|bug| bug.creation_time.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::LastChangeTime,
        label: "Updated",
        value: DetailValue::OptionalText(|bug| bug.last_change_time.as_deref()),
    },
    BuiltinDetailField {
        field: BugField::Keywords,
        label: "Keywords",
        value: DetailValue::StringList(|bug| &bug.keywords),
    },
    BuiltinDetailField {
        field: BugField::Blocks,
        label: "Blocks",
        value: DetailValue::IdList(|bug| &bug.blocks),
    },
    BuiltinDetailField {
        field: BugField::DependsOn,
        label: "Depends on",
        value: DetailValue::IdList(|bug| &bug.depends_on),
    },
    BuiltinDetailField {
        field: BugField::Flags,
        label: "Flags",
        value: DetailValue::Flags,
    },
];

fn render_builtin_detail_field(field: BuiltinDetailField, bug: &Bug) -> Option<DetailRow> {
    let value = match field.value {
        DetailValue::Status => bug.status.as_deref().map_or_else(
            || "-".to_string(),
            |status| colorize_status(status).to_string(),
        ),
        DetailValue::OptionalText(accessor) => accessor(bug).unwrap_or("-").to_string(),
        DetailValue::OptionalId(accessor) => accessor(bug)?.to_string(),
        DetailValue::StringList(accessor) => {
            let values = accessor(bug);
            if values.is_empty() {
                return None;
            }
            values.join(", ")
        }
        DetailValue::IdList(accessor) => {
            let values = accessor(bug);
            if values.is_empty() {
                return None;
            }
            join_ids(values)
        }
        DetailValue::TargetMilestone => {
            let milestone = bug.target_milestone.as_deref().unwrap_or_default();
            if milestone.is_empty() || milestone == UNSET_MILESTONE {
                return None;
            }
            milestone.to_string()
        }
        DetailValue::Flags => {
            if bug.flags.is_empty() {
                return None;
            }
            render_flags_inline(&bug.flags)
        }
    };
    Some(DetailRow {
        label: field.label,
        value,
    })
}

fn write_bug_detail_table(bug: &Bug, spec: ColumnSpec<'_>, out: &mut (impl Write + ?Sized)) {
    if field_selected(spec, "summary") {
        let _ = writeln!(
            out,
            "{} #{}\n{}\n",
            "Bug".bold(),
            bug.id.to_string().bold(),
            bug.summary.as_deref().unwrap_or("unknown").bold()
        );
    } else {
        let _ = writeln!(out, "{} #{}\n", "Bug".bold(), bug.id.to_string().bold());
    }
    for detail in BUILTIN_DETAIL_FIELDS {
        if !field_selected(spec, detail.field.canonical()) {
            continue;
        }
        if let Some(row) = render_builtin_detail_field(*detail, bug) {
            write_field(out, row.label, &row.value);
        }
    }
    for name in selected_custom_detail_fields(spec) {
        write_field(out, name, &render_custom_value(bug.custom_fields.get(name)));
    }
}

/// Render bug history as the human-readable table: one colorized block per
/// history entry with its nested field changes. JSON/ndjson output goes through
/// [`write_history_json`] instead, which emits flattened per-field records.
pub fn write_history_table<W: Write + ?Sized>(history: &[HistoryEntry], out: &mut W) {
    disable_color_for_tests();
    for entry in history {
        let _ = writeln!(
            out,
            "{} by {} ({})",
            "Change".bold(),
            entry.who.cyan(),
            entry.when,
        );
        for change in &entry.changes {
            let attachment_suffix = change
                .attachment_id
                .map(|id| format!(" [attachment #{id}]"))
                .unwrap_or_default();
            let _ = writeln!(out, "  {}{attachment_suffix}:", change.field_name.bold());
            if let Some(removed) = change.removed.as_deref().filter(|value| !value.is_empty()) {
                let _ = writeln!(out, "    - {}", removed.red());
            }
            if let Some(added) = change.added.as_deref().filter(|value| !value.is_empty()) {
                let _ = writeln!(out, "    + {}", added.green());
            }
        }
        write_divider(out);
    }
}

/// Render bug history as flattened change records (one per changed field) for the
/// JSON family. An empty slice emits an empty array (`--json`) or nothing
/// (`--output ndjson`), per the shared `write_json_family` contract — agents
/// always receive valid JSON. See ADR 0008 and `schemas/history.json`.
pub fn write_history_json<W: Write + ?Sized>(
    records: &[HistoryRecord],
    format: OutputFormat,
    out: &mut W,
) {
    write_json_family(records, format, out);
}

/// Render relationship records (`bug links`). JSON/ndjson serialize the slice;
/// table mode prints a fixed six-column grid. An empty slice renders nothing in
/// every mode — the command prints the "no related bugs" line so it can name the
/// root id.
pub fn write_bug_links<W: Write + ?Sized>(links: &[BugLink], format: OutputFormat, out: &mut W) {
    write_formatted(links, format, out, |links, out| {
        if links.is_empty() {
            return;
        }
        let mut builder = Builder::default();
        builder.push_record(["ID", "RELATION", "DIR", "DEPTH", "STATUS", "SUMMARY"]);
        for link in links {
            builder.push_record([
                link.id.to_string(),
                link.relation.as_str().to_string(),
                link.direction.as_str().to_string(),
                link.depth.to_string(),
                link.status.clone().unwrap_or_default(),
                link.summary
                    .as_deref()
                    .map(|s| truncate(s, SUMMARY_TRUNCATE_WIDTH))
                    .unwrap_or_default(),
            ]);
        }
        let _ = writeln!(out, "{}", builder.build());
    });
}

/// Render a bounded adjacency result as a structured payload or two fixed table sections.
pub fn write_bug_adjacency<W: Write + ?Sized>(
    result: &mut BugAdjacencyResult,
    format: OutputFormat,
    out: &mut W,
) {
    disable_color_for_tests();
    normalize_bug_adjacency(result);
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => write_json_family(result, format, out),
        OutputFormat::Table => write_bug_adjacency_table(result, out),
    }
}

fn normalize_bug_adjacency(result: &mut BugAdjacencyResult) {
    for bug in &mut result.bugs {
        bug.blocks.sort_unstable();
        bug.blocks.dedup();
        bug.depends_on.sort_unstable();
        bug.depends_on.dedup();
    }
}

fn write_bug_adjacency_table(result: &BugAdjacencyResult, out: &mut (impl Write + ?Sized)) {
    let _ = writeln!(out, "Requests");
    let mut requests = Builder::default();
    requests.push_record(["REQUESTED", "RESULT"]);
    for request in &result.requests {
        let (requested, outcome) = match request {
            BugAdjacencyRequest::Success { requested, bug_id } => (requested, bug_id.to_string()),
            BugAdjacencyRequest::Failure { requested, error } => {
                (requested, format_adjacency_error(*error))
            }
        };
        requests.push_record([requested.clone(), outcome]);
    }
    let _ = writeln!(out, "{}", requests.build());

    let _ = writeln!(out, "\nCanonical bugs");
    let mut bugs = Builder::default();
    bugs.push_record([
        "ID",
        "SUMMARY",
        "STATUS",
        "RESOLUTION",
        "PRODUCT",
        "VERSION",
        "ASSIGNEE",
        "LAST CHANGE TIME",
        "TARGET MILESTONE",
        "BLOCKS",
        "DEPENDS ON",
    ]);
    for bug in &result.bugs {
        bugs.push_record([
            bug.id.to_string(),
            bug.summary.clone().unwrap_or_default(),
            bug.status.clone().unwrap_or_default(),
            bug.resolution.clone().unwrap_or_default(),
            bug.product.clone().unwrap_or_default(),
            bug.version.clone().unwrap_or_default(),
            bug.assigned_to.clone().unwrap_or_default(),
            bug.last_change_time.clone().unwrap_or_default(),
            bug.target_milestone.clone().unwrap_or_default(),
            join_ids(&bug.blocks),
            join_ids(&bug.depends_on),
        ]);
    }
    let _ = writeln!(out, "{}", bugs.build());
}

fn format_adjacency_error(error: BugAdjacencyError) -> String {
    let label = match error {
        BugAdjacencyError::NotFoundAlias | BugAdjacencyError::NotFoundId => "NOT FOUND",
        BugAdjacencyError::Inaccessible => "INACCESSIBLE",
    };
    format!("{label} ({})", error.api_code())
}

/// One row in a multi-ID `bzr bug view` output stream.
///
/// Used by [`write_multi_bug_view`] to interleave successful detail
/// blocks with `UNAVAILABLE` placeholder blocks for inaccessible bugs.
#[non_exhaustive]
#[derive(Debug)]
pub enum MultiBugRow {
    Ok(Box<Bug>),
    Failed { id: String, error: String },
}

/// Render a multi-ID `bzr bug view` result.
///
/// JSON mode is **not** handled here — the caller builds the
/// `{"bugs": [...], "failed": [...]}` wrapper itself (projecting each bug via
/// [`bug_to_json`]). This function only covers table mode: argument-order
/// detail blocks for `Ok`, visually distinct `UNAVAILABLE` placeholder blocks
/// for `Failed`, with a `─`-divider line between every pair of blocks (no
/// trailing divider).
pub fn write_multi_bug_view<W: Write + ?Sized>(
    rows: &[MultiBugRow],
    spec: ColumnSpec<'_>,
    out: &mut W,
) {
    disable_color_for_tests();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            write_divider(out);
        }
        match row {
            MultiBugRow::Ok(bug) => write_bug_detail_table(bug, spec, out),
            MultiBugRow::Failed { id, error } => write_unavailable_block(id, error, out),
        }
    }
}

fn write_unavailable_block(id: &str, error: &str, out: &mut (impl Write + ?Sized)) {
    let _ = writeln!(
        out,
        "{} #{} — {}",
        "Bug".bold(),
        id.bold(),
        "UNAVAILABLE".red().bold()
    );
    let _ = writeln!(out, "  Error: {error}");
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
