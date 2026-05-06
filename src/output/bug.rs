use std::io::{self, Write};

use colored::Colorize;
use tabled::{Table, Tabled};

use super::formatting::{colorize_status, print_formatted, shorten_email, truncate};
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

pub fn print_bugs(bugs: &[Bug], format: OutputFormat) {
    print_formatted(bugs, format, |bugs| {
        if bugs.is_empty() {
            let _ = writeln!(io::stdout(), "No bugs found.");
            return;
        }
        let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
        let table = Table::new(rows).to_string();
        let _ = writeln!(io::stdout(), "{table}");
    });
}

pub fn print_bug_detail(bug: &Bug, format: OutputFormat) {
    print_formatted(bug, format, |bug| {
        write_bug_detail(bug, &mut io::stdout());
    });
}

fn write_bug_detail(bug: &Bug, out: &mut impl Write) {
    let _ = writeln!(
        out,
        "{} #{}\n{}\n",
        "Bug".bold(),
        bug.id.to_string().bold(),
        bug.summary.bold()
    );
    write_field(out, "Status", &colorize_status(&bug.status));
    write_optional_field(out, "Resolution", bug.resolution.as_deref());
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

fn write_field(out: &mut impl Write, label: &str, value: &str) {
    let _ = writeln!(out, "  {label:<12}  {value}");
}

fn write_optional_field(out: &mut impl Write, label: &str, value: Option<&str>) {
    let _ = writeln!(out, "  {label:<12}  {}", value.unwrap_or("-"));
}

fn write_list_field(out: &mut impl Write, label: &str, items: &[String]) {
    if !items.is_empty() {
        let _ = writeln!(out, "  {label:<12}  {}", items.join(", "));
    }
}

fn write_id_list_field(out: &mut impl Write, label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        let id_str = ids
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  {label:<12}  {id_str}");
    }
}

pub fn print_history(history: &[HistoryEntry], format: OutputFormat) {
    print_formatted(history, format, |history| {
        for entry in history {
            let _ = writeln!(
                io::stdout(),
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
                let _ = writeln!(
                    io::stdout(),
                    "  {}{attachment_suffix}:",
                    change.field_name.bold()
                );
                if !change.removed.is_empty() {
                    let _ = writeln!(io::stdout(), "    - {}", change.removed.red());
                }
                if !change.added.is_empty() {
                    let _ = writeln!(io::stdout(), "    + {}", change.added.green());
                }
            }
            let _ = writeln!(io::stdout(), "{}", "─".repeat(60));
        }
    });
}

/// One row in a multi-ID `bzr bug view` output stream.
///
/// Used by [`print_multi_bug_view`] to interleave successful detail
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
/// `MultiBugViewResult` via `output::print_result`. This function only
/// covers table mode: argument-order detail blocks for `Ok`, visually
/// distinct `UNAVAILABLE` placeholder blocks for `Failed`, with a
/// `─`-divider line between every pair of blocks (no trailing divider).
pub fn print_multi_bug_view(rows: &[MultiBugRow]) {
    write_multi_bug_view(rows, &mut io::stdout());
}

fn write_multi_bug_view(rows: &[MultiBugRow], out: &mut impl Write) {
    let divider = "─".repeat(60);
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(out, "{divider}");
        }
        match row {
            MultiBugRow::Ok(bug) => write_bug_detail(bug, out),
            MultiBugRow::Failed { id, error } => write_unavailable_block(id, error, out),
        }
    }
}

fn write_unavailable_block(id: &str, error: &str, out: &mut impl Write) {
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
