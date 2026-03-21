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

#[expect(clippy::print_stdout)]
pub fn print_bugs(bugs: &[Bug], format: OutputFormat) {
    print_formatted(bugs, format, |bugs| {
        if bugs.is_empty() {
            println!("No bugs found.");
            return;
        }
        let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
        let table = Table::new(rows).to_string();
        println!("{table}");
    });
}

#[expect(clippy::print_stdout)]
pub fn print_bug_detail(bug: &Bug, format: OutputFormat) {
    print_formatted(bug, format, |bug| {
        println!(
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

#[expect(clippy::print_stdout)]
pub fn print_history(history: &[HistoryEntry], format: OutputFormat) {
    print_formatted(history, format, |history| {
        for entry in history {
            println!(
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
                println!("  {}{attachment_suffix}:", change.field_name.bold(),);
                if !change.removed.is_empty() {
                    println!("    - {}", change.removed.red());
                }
                if !change.added.is_empty() {
                    println!("    + {}", change.added.green());
                }
            }
            println!("{}", "─".repeat(60));
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::{Bug, FieldChange, HistoryEntry};
    use tabled::Table;

    fn make_bug(id: u64, summary: &str, status: &str) -> Bug {
        Bug {
            id,
            summary: summary.into(),
            status: status.into(),
            resolution: None,
            product: Some("TestProduct".into()),
            component: Some("TestComponent".into()),
            version: Some("1.0".into()),
            assigned_to: Some("dev@example.com".into()),
            priority: Some("P1".into()),
            severity: Some("major".into()),
            creation_time: Some("2025-01-15T10:00:00Z".into()),
            last_change_time: Some("2025-01-16T12:00:00Z".into()),
            creator: Some("reporter@example.com".into()),
            url: None,
            whiteboard: None,
            keywords: vec!["regression".into()],
            blocks: vec![200, 201],
            depends_on: vec![100],
            cc: vec!["watcher@example.com".into()],
            op_sys: None,
            rep_platform: None,
        }
    }

    fn make_history_entry() -> HistoryEntry {
        HistoryEntry {
            who: "editor@example.com".into(),
            when: "2025-04-01T12:00:00Z".into(),
            changes: vec![
                FieldChange {
                    field_name: "status".into(),
                    removed: "NEW".into(),
                    added: "ASSIGNED".into(),
                    attachment_id: None,
                },
                FieldChange {
                    field_name: "flagtypes.name".into(),
                    removed: String::new(),
                    added: "review?".into(),
                    attachment_id: Some(99),
                },
            ],
        }
    }

    // ── BugRow conversion ────────────────────────────────────────────

    #[test]
    fn bug_row_from_bug_truncates_summary() {
        let mut bug = make_bug(1, &"x".repeat(100), "NEW");
        let row = BugRow::from(&bug);
        assert_eq!(row.summary.chars().count(), 72);
        assert!(row.summary.ends_with("..."));

        bug.summary = "short".into();
        let row = BugRow::from(&bug);
        assert_eq!(row.summary, "short");
    }

    #[test]
    fn bug_row_from_bug_shortens_assignee() {
        let bug = make_bug(1, "test", "NEW");
        let row = BugRow::from(&bug);
        assert_eq!(row.assignee, "dev");
    }

    #[test]
    fn bug_row_from_bug_missing_fields() {
        let bug = Bug {
            id: 1,
            summary: "minimal".into(),
            status: "NEW".into(),
            resolution: None,
            product: None,
            component: None,
            version: None,
            assigned_to: None,
            priority: None,
            severity: None,
            creation_time: None,
            last_change_time: None,
            creator: None,
            url: None,
            whiteboard: None,
            keywords: vec![],
            blocks: vec![],
            depends_on: vec![],
            cc: vec![],
            op_sys: None,
            rep_platform: None,
        };
        let row = BugRow::from(&bug);
        assert_eq!(row.priority, "");
        assert_eq!(row.assignee, "");
    }

    // ── print_bugs ───────────────────────────────────────────────────

    #[test]
    fn print_bugs_json_empty_list() {
        let bugs: Vec<Bug> = vec![];
        let json = serde_json::to_string_pretty(&bugs).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_bugs_json_one_bug() {
        let bugs = vec![make_bug(42, "Login broken", "NEW")];
        let json = serde_json::to_string_pretty(&bugs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["id"], 42);
        assert_eq!(parsed[0]["summary"], "Login broken");
    }

    #[test]
    fn print_bugs_table_renders_rows() {
        let bugs = [make_bug(42, "Login broken", "NEW")];
        let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
        let table = Table::new(rows).to_string();
        assert!(table.contains("42"));
        assert!(table.contains("NEW"));
        assert!(table.contains("Login broken"));
        assert!(table.contains("ID"));
        assert!(table.contains("STATUS"));
        assert!(table.contains("SUMMARY"));
    }

    // ── print_bug_detail ─────────────────────────────────────────────

    #[test]
    fn print_bug_detail_json_contains_all_fields() {
        let bug = make_bug(42, "Detail test", "ASSIGNED");
        let json = serde_json::to_string_pretty(&bug).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["summary"], "Detail test");
        assert_eq!(parsed["status"], "ASSIGNED");
        assert_eq!(parsed["product"], "TestProduct");
        assert_eq!(parsed["component"], "TestComponent");
        assert_eq!(parsed["assigned_to"], "dev@example.com");
        assert_eq!(parsed["priority"], "P1");
        assert_eq!(parsed["severity"], "major");
        assert_eq!(parsed["creator"], "reporter@example.com");
        assert_eq!(parsed["keywords"][0], "regression");
        assert_eq!(parsed["blocks"][0], 200);
        assert_eq!(parsed["depends_on"][0], 100);
    }

    #[test]
    fn print_bug_detail_json_with_resolution() {
        let mut bug = make_bug(42, "Fixed bug", "RESOLVED");
        bug.resolution = Some("FIXED".into());
        let json = serde_json::to_string_pretty(&bug).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["resolution"], "FIXED");
    }

    // ── print_json helper ────────────────────────────────────────────

    #[test]
    fn print_json_produces_valid_json_for_bug() {
        let bug = make_bug(1, "Test bug", "NEW");
        let json = serde_json::to_string_pretty(&bug).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["summary"], "Test bug");
        assert_eq!(parsed["status"], "NEW");
    }

    #[test]
    fn print_json_produces_valid_json_for_vec() {
        let bugs = vec![make_bug(1, "A", "NEW"), make_bug(2, "B", "RESOLVED")];
        let json = serde_json::to_string_pretty(&bugs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    // ── print_history ────────────────────────────────────────────────

    #[test]
    fn print_history_json_one_entry() {
        let history = vec![make_history_entry()];
        let json = serde_json::to_string_pretty(&history).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["who"], "editor@example.com");
        assert_eq!(parsed[0]["when"], "2025-04-01T12:00:00Z");
        let changes = parsed[0]["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0]["field_name"], "status");
        assert_eq!(changes[0]["removed"], "NEW");
        assert_eq!(changes[0]["added"], "ASSIGNED");
        assert_eq!(changes[1]["attachment_id"], 99);
    }

    #[test]
    fn print_history_json_empty() {
        let history: Vec<HistoryEntry> = vec![];
        let json = serde_json::to_string_pretty(&history).unwrap();
        assert_eq!(json, "[]");
    }
}
