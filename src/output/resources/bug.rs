use std::collections::HashSet;
use std::io::Write;

use colored::Colorize;
use serde_json::Value;
use tabled::builder::Builder;

use crate::output::formatting::{
    colorize_status, render_flags_inline, shorten_email, truncate, write_divider, write_field,
    write_formatted, write_json_family, write_list_field, write_optional_field,
    SUMMARY_TRUNCATE_WIDTH,
};
use crate::types::bug::{Bug, HistoryEntry};
use crate::types::bug_fields::{
    apply_exclude, canonical_excludes, canonical_field_list, default_selected_fields,
    field_selected, partition_include, selected_custom_detail_fields, BugField, ColumnSpec,
    SelectedBugField,
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
        BugField::Status => bug.status.clone(),
        BugField::Priority => bug.priority.clone().unwrap_or_default(),
        BugField::AssignedTo => shorten_email(bug.assigned_to.as_deref().unwrap_or("")),
        BugField::Summary => truncate(&bug.summary, SUMMARY_TRUNCATE_WIDTH),
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
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_json_family(&bug_to_json(bug, spec), format, out);
        }
        OutputFormat::Table => write_bug_detail_table(bug, spec, out),
    }
}

fn write_bug_detail_table(bug: &Bug, spec: ColumnSpec<'_>, out: &mut (impl Write + ?Sized)) {
    if field_selected(spec, "summary") {
        let _ = writeln!(
            out,
            "{} #{}\n{}\n",
            "Bug".bold(),
            bug.id.to_string().bold(),
            bug.summary.bold()
        );
    } else {
        let _ = writeln!(out, "{} #{}\n", "Bug".bold(), bug.id.to_string().bold());
    }
    if field_selected(spec, "status") {
        write_field(out, "Status", &colorize_status(&bug.status).to_string());
    }
    if field_selected(spec, "resolution") {
        write_optional_field(out, "Resolution", bug.resolution.as_deref());
    }
    if field_selected(spec, "dupe_of") {
        if let Some(dupe_of) = bug.dupe_of {
            let _ = writeln!(out, "  {:<12}  {dupe_of}", "Duplicate of");
        }
    }
    if field_selected(spec, "product") {
        write_optional_field(out, "Product", bug.product.as_deref());
    }
    if field_selected(spec, "component") {
        write_optional_field(out, "Component", bug.component.as_deref());
    }
    if field_selected(spec, "target_milestone") {
        let milestone = bug.target_milestone.as_deref().unwrap_or_default();
        if !milestone.is_empty() && milestone != UNSET_MILESTONE {
            write_field(out, "Target Milestone", milestone);
        }
    }
    if field_selected(spec, "assigned_to") {
        write_optional_field(out, "Assignee", bug.assigned_to.as_deref());
    }
    if field_selected(spec, "priority") {
        write_optional_field(out, "Priority", bug.priority.as_deref());
    }
    if field_selected(spec, "severity") {
        write_optional_field(out, "Severity", bug.severity.as_deref());
    }
    if field_selected(spec, "creator") {
        write_optional_field(out, "Creator", bug.creator.as_deref());
    }
    if field_selected(spec, "creation_time") {
        write_optional_field(out, "Created", bug.creation_time.as_deref());
    }
    if field_selected(spec, "last_change_time") {
        write_optional_field(out, "Updated", bug.last_change_time.as_deref());
    }
    if field_selected(spec, "keywords") {
        write_list_field(out, "Keywords", &bug.keywords);
    }
    if field_selected(spec, "blocks") {
        write_id_list_field(out, "Blocks", &bug.blocks);
    }
    if field_selected(spec, "depends_on") {
        write_id_list_field(out, "Depends on", &bug.depends_on);
    }
    if field_selected(spec, "flags") && !bug.flags.is_empty() {
        write_field(out, "Flags", &render_flags_inline(&bug.flags));
    }
    for name in selected_custom_detail_fields(spec) {
        write_field(out, name, &render_custom_value(bug.custom_fields.get(name)));
    }
}

fn write_id_list_field(out: &mut (impl Write + ?Sized), label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        let id_str = join_ids(ids);
        let _ = writeln!(out, "  {label:<12}  {id_str}");
    }
}

pub fn write_history<W: Write + ?Sized>(
    history: &[HistoryEntry],
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(history, format, out, |history, out| {
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
                if !change.removed.is_empty() {
                    let _ = writeln!(out, "    - {}", change.removed.red());
                }
                if !change.added.is_empty() {
                    let _ = writeln!(out, "    + {}", change.added.green());
                }
            }
            write_divider(out);
        }
    });
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
