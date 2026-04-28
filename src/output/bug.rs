use std::io::{self, Write as _};

use colored::Colorize;
use tabled::{Table, Tabled};

use super::formatting::{
    colorize_status, print_field, print_formatted, print_id_list_field, print_list_field,
    print_optional_field, shorten_email, truncate,
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
        let _ = writeln!(
            io::stdout(),
            "{} #{}\n{}\n",
            "Bug".bold(),
            bug.id.to_string().bold(),
            bug.summary.bold()
        );
        print_field("Status", &colorize_status(&bug.status));
        print_optional_field("Resolution", bug.resolution.as_deref());
        print_optional_field("Product", bug.product.as_deref());
        print_optional_field("Component", bug.component.as_deref());
        print_optional_field("Assignee", bug.assigned_to.as_deref());
        print_optional_field("Priority", bug.priority.as_deref());
        print_optional_field("Severity", bug.severity.as_deref());
        print_optional_field("Creator", bug.creator.as_deref());
        print_optional_field("Created", bug.creation_time.as_deref());
        print_optional_field("Updated", bug.last_change_time.as_deref());
        print_list_field("Keywords", &bug.keywords);
        print_id_list_field("Blocks", &bug.blocks);
        print_id_list_field("Depends on", &bug.depends_on);
    });
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

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
