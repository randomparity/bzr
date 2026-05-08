use std::io::Write;

use colored::Colorize;
use tabled::{Table, Tabled};

use crate::output::formatting::{
    colorize_status, shorten_email, truncate, write_divider, write_field, write_formatted,
    write_list_field, write_optional_field,
};
use crate::types::{Bug, HistoryEntry, OutputFormat};

#[derive(Tabled)]
struct BugRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "PRIORITY")]
    priority: String,
    #[tabled(rename = "ASSIGNEE")]
    assignee: String,
    #[tabled(rename = "SUMMARY")]
    summary: String,
}

impl From<&Bug> for BugRow {
    fn from(b: &Bug) -> Self {
        let summary = truncate(&b.summary, 72);
        BugRow {
            id: b.id,
            status: b.status.clone(),
            priority: b.priority.clone().unwrap_or_default(),
            assignee: shorten_email(b.assigned_to.as_deref().unwrap_or("")),
            summary,
        }
    }
}

pub fn write_bugs<W: Write + ?Sized>(bugs: &[Bug], format: OutputFormat, out: &mut W) {
    write_formatted(bugs, format, out, |bugs, out| {
        if bugs.is_empty() {
            let _ = writeln!(out, "No bugs found.");
            return;
        }
        let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
        let table = Table::new(rows).to_string();
        let _ = writeln!(out, "{table}");
    });
}

pub fn write_bug_detail<W: Write + ?Sized>(bug: &Bug, format: OutputFormat, out: &mut W) {
    write_formatted(bug, format, out, |bug, out| {
        write_bug_detail_table(bug, out);
    });
}

fn write_bug_detail_table(bug: &Bug, out: &mut (impl Write + ?Sized)) {
    let _ = writeln!(
        out,
        "{} #{}\n{}\n",
        "Bug".bold(),
        bug.id.to_string().bold(),
        bug.summary.bold()
    );
    write_field(out, "Status", &colorize_status(&bug.status));
    write_optional_field(out, "Resolution", bug.resolution.as_deref());
    if let Some(dupe_of) = bug.dupe_of {
        let _ = writeln!(out, "  {:<12}  {dupe_of}", "Duplicate of");
    }
    write_optional_field(out, "Product", bug.product.as_deref());
    write_optional_field(out, "Component", bug.component.as_deref());
    write_optional_field(out, "Assignee", bug.assigned_to.as_deref());
    write_optional_field(out, "Priority", bug.priority.as_deref());
    write_optional_field(out, "Severity", bug.severity.as_deref());
    write_optional_field(out, "Creator", bug.creator.as_deref());
    write_optional_field(out, "Created", bug.creation_time.as_deref());
    write_optional_field(out, "Updated", bug.last_change_time.as_deref());
    write_list_field(out, "Keywords", &bug.keywords);
    write_id_list_field(out, "Blocks", &bug.blocks);
    write_id_list_field(out, "Depends on", &bug.depends_on);
}

fn write_id_list_field(out: &mut (impl Write + ?Sized), label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        let id_str = ids
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
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
/// JSON mode is **not** handled here — the caller emits a
/// `MultiBugViewResult` via `output::write_result`. This function only
/// covers table mode: argument-order detail blocks for `Ok`, visually
/// distinct `UNAVAILABLE` placeholder blocks for `Failed`, with a
/// `─`-divider line between every pair of blocks (no trailing divider).
pub fn write_multi_bug_view<W: Write + ?Sized>(rows: &[MultiBugRow], out: &mut W) {
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            write_divider(out);
        }
        match row {
            MultiBugRow::Ok(bug) => write_bug_detail_table(bug, out),
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
