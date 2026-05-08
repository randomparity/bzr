#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::{Bug, FieldChange, HistoryEntry};
use tabled::Table;

fn make_bug(id: u64, summary: &str, status: &str) -> Bug {
    Bug {
        id,
        summary: summary.into(),
        status: status.into(),
        resolution: None,
        dupe_of: None,
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

fn capture_bugs(format: OutputFormat, bugs: &[Bug]) -> String {
    let mut buf = Vec::new();
    write_bugs(bugs, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_bug_detail(format: OutputFormat, bug: &Bug) -> String {
    let mut buf = Vec::new();
    write_bug_detail(bug, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_history(format: OutputFormat, history: &[HistoryEntry]) -> String {
    let mut buf = Vec::new();
    write_history(history, format, &mut buf);
    String::from_utf8(buf).unwrap()
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
        dupe_of: None,
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

// ── write_bugs ───────────────────────────────────────────────────

#[test]
fn write_bugs_json_empty_list() {
    let bugs: Vec<Bug> = vec![];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_bugs_json_one_bug() {
    let bugs = vec![make_bug(42, "Login broken", "NEW")];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["id"], 42);
    assert_eq!(parsed[0]["summary"], "Login broken");
}

#[test]
fn write_bugs_table_renders_rows() {
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

#[test]
fn write_bugs_table_empty_says_no_bugs_found() {
    let output = capture_bugs(OutputFormat::Table, &[]);
    assert!(output.contains("No bugs found."));
}

#[test]
fn write_bugs_json_empty_renders_empty_array() {
    let output = capture_bugs(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_bugs_table_renders_columns_and_truncates() {
    let bugs = vec![make_bug(42, "Login broken", "NEW")];
    let output = capture_bugs(OutputFormat::Table, &bugs);
    assert!(output.contains("ID"));
    assert!(output.contains("STATUS"));
    assert!(output.contains("PRIORITY"));
    assert!(output.contains("ASSIGNEE"));
    assert!(output.contains("SUMMARY"));
    assert!(output.contains("42"));
    assert!(output.contains("NEW"));
    assert!(output.contains("P1"));
    assert!(output.contains("dev"));
    assert!(output.contains("Login broken"));
}

#[test]
fn write_bugs_json_via_write() {
    let bugs = vec![make_bug(99, "Crash on startup", "ASSIGNED")];
    let output = capture_bugs(OutputFormat::Json, &bugs);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 99);
    assert_eq!(parsed[0]["summary"], "Crash on startup");
    assert_eq!(parsed[0]["status"], "ASSIGNED");
}

// ── write_bug_detail ─────────────────────────────────────────────

#[test]
fn write_bug_detail_json_contains_all_fields() {
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
fn write_bug_detail_json_with_resolution() {
    let mut bug = make_bug(42, "Fixed bug", "RESOLVED");
    bug.resolution = Some("FIXED".into());
    let json = serde_json::to_string_pretty(&bug).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["resolution"], "FIXED");
}

#[test]
fn write_bug_detail_table_renders_all_fields() {
    let mut bug = make_bug(42, "Detail test", "ASSIGNED");
    bug.resolution = Some("FIXED".into());
    let output = capture_bug_detail(OutputFormat::Table, &bug);
    assert!(output.contains("Bug"));
    assert!(output.contains("#42"));
    assert!(output.contains("Detail test"));
    assert!(output.contains("Status"));
    assert!(output.contains("Resolution"));
    assert!(output.contains("FIXED"));
    assert!(output.contains("Product"));
    assert!(output.contains("TestProduct"));
    assert!(output.contains("Component"));
    assert!(output.contains("TestComponent"));
    assert!(output.contains("Assignee"));
    assert!(output.contains("dev@example.com"));
    assert!(output.contains("Priority"));
    assert!(output.contains("P1"));
    assert!(output.contains("Severity"));
    assert!(output.contains("major"));
    assert!(output.contains("Creator"));
    assert!(output.contains("reporter@example.com"));
    assert!(output.contains("Keywords"));
    assert!(output.contains("regression"));
    assert!(output.contains("Blocks"));
    assert!(output.contains("200, 201"));
    assert!(output.contains("Depends on"));
    assert!(output.contains("100"));
}

#[test]
fn write_bug_detail_table_handles_minimal_bug() {
    let bug = Bug {
        id: 1,
        summary: "Unicode summary — déjà vu".into(),
        status: "NEW".into(),
        resolution: None,
        dupe_of: None,
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
    let output = capture_bug_detail(OutputFormat::Table, &bug);
    assert!(output.contains("Unicode summary — déjà vu"));
    assert!(output.contains('-'));
    assert!(!output.contains("Keywords"));
    assert!(!output.contains("Blocks"));
    assert!(!output.contains("Depends on"));
}

#[test]
fn write_bug_detail_json_via_write() {
    let bug = make_bug(7, "Json bug", "NEW");
    let output = capture_bug_detail(OutputFormat::Json, &bug);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["id"], 7);
    assert_eq!(parsed["summary"], "Json bug");
}

// ── write_json (pretty) ──────────────────────────────────────────

#[test]
fn write_json_produces_valid_json_for_bug() {
    let bug = make_bug(1, "Test bug", "NEW");
    let json = serde_json::to_string_pretty(&bug).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["summary"], "Test bug");
    assert_eq!(parsed["status"], "NEW");
}

#[test]
fn write_json_produces_valid_json_for_vec() {
    let bugs = vec![make_bug(1, "A", "NEW"), make_bug(2, "B", "RESOLVED")];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

// ── write_history ────────────────────────────────────────────────

#[test]
fn write_history_json_one_entry() {
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
fn write_history_json_empty() {
    let history: Vec<HistoryEntry> = vec![];
    let json = serde_json::to_string_pretty(&history).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_history_table_renders_changes() {
    let history = vec![make_history_entry()];
    let output = capture_history(OutputFormat::Table, &history);
    assert!(output.contains("Change"));
    assert!(output.contains("editor@example.com"));
    assert!(output.contains("2025-04-01T12:00:00Z"));
    assert!(output.contains("status"));
    assert!(output.contains("NEW"));
    assert!(output.contains("ASSIGNED"));
    assert!(output.contains("flagtypes.name"));
    assert!(output.contains("[attachment #99]"));
    assert!(output.contains("review?"));
    assert!(output.contains('─'));
}

#[test]
fn write_history_table_empty_renders_nothing() {
    let output = capture_history(OutputFormat::Table, &[]);
    assert!(!output.contains("Change"));
    assert!(!output.contains('─'));
}

#[test]
fn write_history_json_via_write() {
    let history = vec![make_history_entry()];
    let output = capture_history(OutputFormat::Json, &history);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["who"], "editor@example.com");
}

// ── multi_bug_view ───────────────────────────────────────────────

fn no_color() {
    colored::control::set_override(false);
}

fn sample_bug(id: u64, summary: &str) -> Bug {
    Bug {
        id,
        summary: summary.into(),
        status: "NEW".into(),
        resolution: None,
        dupe_of: None,
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
    }
}

#[test]
fn multi_bug_view_renders_success_blocks_with_dividers() {
    no_color();
    let rows = vec![
        MultiBugRow::Ok(Box::new(sample_bug(1, "first"))),
        MultiBugRow::Ok(Box::new(sample_bug(2, "second"))),
        MultiBugRow::Ok(Box::new(sample_bug(3, "third"))),
    ];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Bug #1"));
    assert!(out.contains("Bug #2"));
    assert!(out.contains("Bug #3"));
    let divider = "─".repeat(60);
    assert_eq!(out.matches(&divider).count(), 2);
}

#[test]
fn multi_bug_view_renders_failure_block_with_unavailable_marker() {
    no_color();
    let rows = vec![MultiBugRow::Failed {
        id: "999".into(),
        error: "bug not found: 999".into(),
    }];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Bug #999"));
    assert!(out.contains("UNAVAILABLE"));
    assert!(out.contains("Error: bug not found: 999"));
}

#[test]
fn multi_bug_view_single_row_emits_no_divider() {
    no_color();
    let rows = vec![MultiBugRow::Ok(Box::new(sample_bug(7, "only")))];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    let divider = "─".repeat(60);
    assert_eq!(out.matches(&divider).count(), 0);
}

#[test]
fn multi_bug_view_interleaves_success_and_failure_in_order() {
    no_color();
    let rows = vec![
        MultiBugRow::Ok(Box::new(sample_bug(10, "alpha"))),
        MultiBugRow::Failed {
            id: "11".into(),
            error: "denied".into(),
        },
        MultiBugRow::Ok(Box::new(sample_bug(12, "gamma"))),
    ];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    let pos_alpha = out.find("Bug #10").unwrap();
    let pos_unavail = out.find("Bug #11").unwrap();
    let pos_gamma = out.find("Bug #12").unwrap();
    assert!(pos_alpha < pos_unavail && pos_unavail < pos_gamma);
}
