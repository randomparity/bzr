#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::{BugzillaUser, UserGroup, WhoamiResponse};
use tabled::Table;

fn make_user(id: u64, name: &str, can_login: Option<bool>, groups: Vec<&str>) -> BugzillaUser {
    BugzillaUser {
        id,
        name: name.into(),
        real_name: Some(format!("{name} Real")),
        email: Some(format!("{name}@example.com")),
        groups: groups
            .into_iter()
            .map(|g| UserGroup {
                id: 1,
                name: g.into(),
                description: String::new(),
            })
            .collect(),
        can_login,
    }
}

fn make_whoami() -> WhoamiResponse {
    WhoamiResponse {
        id: 42,
        name: "testuser".into(),
        real_name: Some("Test User".into()),
        login: Some("testuser@example.com".into()),
    }
}

// ── Existing user row tests ──────────────────────────────────────

#[test]
fn user_row_excludes_detail_columns() {
    let user = make_user(1, "alice", Some(true), vec!["admin"]);
    let row = UserRow {
        id: user.id,
        name: user.name.clone(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
    };
    let table = Table::new(vec![row]).to_string();
    assert!(table.contains("ID"));
    assert!(table.contains("NAME"));
    assert!(table.contains("EMAIL"));
    assert!(!table.contains("CAN LOGIN"));
    assert!(!table.contains("GROUPS"));
}

#[test]
fn detailed_user_row_includes_groups_and_login() {
    let users = [
        make_user(1, "alice", Some(true), vec!["admin", "dev"]),
        make_user(2, "bob", Some(false), vec![]),
        make_user(3, "carol", None, vec!["testers"]),
    ];
    let rows: Vec<DetailedUserRow> = users.iter().map(detailed_row).collect();
    let table = Table::new(rows).to_string();
    assert!(table.contains("CAN LOGIN"));
    assert!(table.contains("GROUPS"));
    assert!(table.contains("Yes"));
    assert!(table.contains("No"));
    assert!(table.contains("admin, dev"));
    assert!(table.contains('-'));
    let lines: Vec<&str> = table.lines().collect();
    let carol_line = lines.iter().find(|l| l.contains("carol")).unwrap();
    assert!(carol_line.contains("testers"));
    assert!(carol_line.contains('-'));
}

#[test]
fn print_users_json_includes_can_login() {
    let users = vec![make_user(1, "alice", Some(true), vec!["admin"])];
    let json = serde_json::to_string_pretty(&users).unwrap();
    assert!(json.contains("\"can_login\": true"));
    assert!(json.contains("\"groups\""));
}

// ── print_whoami ─────────────────────────────────────────────────

#[test]
fn print_whoami_json() {
    let whoami = make_whoami();
    let json = serde_json::to_string_pretty(&whoami).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["name"], "testuser");
    assert_eq!(parsed["real_name"], "Test User");
    assert_eq!(parsed["login"], "testuser@example.com");
}

#[test]
fn print_whoami_json_minimal() {
    let whoami = WhoamiResponse {
        id: 1,
        name: "bot".into(),
        real_name: None,
        login: None,
    };
    let json = serde_json::to_string_pretty(&whoami).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 1);
    assert!(parsed["real_name"].is_null());
    assert!(parsed["login"].is_null());
}

// ── print_users (extended) ───────────────────────────────────────

#[test]
fn print_users_json_empty() {
    let users: Vec<BugzillaUser> = vec![];
    let json = serde_json::to_string_pretty(&users).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn print_users_json_includes_all_fields() {
    let users = vec![make_user(1, "alice", Some(true), vec!["admin"])];
    let json = serde_json::to_string_pretty(&users).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["name"], "alice");
    assert_eq!(parsed[0]["real_name"], "alice Real");
    assert_eq!(parsed[0]["email"], "alice@example.com");
    assert_eq!(parsed[0]["can_login"], true);
    assert_eq!(parsed[0]["groups"][0]["name"], "admin");
}

fn capture_users(format: OutputFormat, users: &[BugzillaUser]) -> String {
    let mut buf = Vec::new();
    write_users(users, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_users_detailed(format: OutputFormat, users: &[BugzillaUser]) -> String {
    let mut buf = Vec::new();
    write_users_detailed(users, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_whoami(format: OutputFormat, whoami: &WhoamiResponse) -> String {
    let mut buf = Vec::new();
    write_whoami(whoami, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_users_table_empty_says_none_found() {
    let output = capture_users(OutputFormat::Table, &[]);
    assert!(output.contains("No users found."));
}

#[test]
fn write_users_json_empty_renders_empty_array() {
    let output = capture_users(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_users_table_renders_basic_columns() {
    let users = vec![make_user(1, "alice", Some(true), vec!["admin"])];
    let output = capture_users(OutputFormat::Table, &users);
    assert!(output.contains("ID"));
    assert!(output.contains("NAME"));
    assert!(output.contains("REAL NAME"));
    assert!(output.contains("EMAIL"));
    assert!(output.contains("alice"));
    assert!(output.contains("alice Real"));
    assert!(output.contains("alice@example.com"));
    assert!(!output.contains("CAN LOGIN"));
    assert!(!output.contains("GROUPS"));
}

#[test]
fn write_users_detailed_table_empty_says_none_found() {
    let output = capture_users_detailed(OutputFormat::Table, &[]);
    assert!(output.contains("No users found."));
}

#[test]
fn write_users_detailed_table_renders_groups_and_login() {
    let users = vec![
        make_user(1, "alice", Some(true), vec!["admin", "dev"]),
        make_user(2, "bob", Some(false), vec![]),
        make_user(3, "carol", None, vec!["testers"]),
    ];
    let output = capture_users_detailed(OutputFormat::Table, &users);
    assert!(output.contains("CAN LOGIN"));
    assert!(output.contains("GROUPS"));
    assert!(output.contains("Yes"));
    assert!(output.contains("No"));
    assert!(output.contains("admin, dev"));
    assert!(output.contains('-'));
}

#[test]
fn write_users_detailed_handles_missing_real_name_and_email() {
    let users = vec![BugzillaUser {
        id: 99,
        name: "minimal".into(),
        real_name: None,
        email: None,
        groups: vec![],
        can_login: None,
    }];
    let output = capture_users_detailed(OutputFormat::Table, &users);
    let data_row = output
        .lines()
        .find(|line| line.contains("minimal"))
        .expect("data row for 'minimal' user");
    let dash_cells = data_row
        .split('|')
        .filter(|cell| cell.trim() == "-")
        .count();
    assert_eq!(
        dash_cells, 2,
        "expected 2 dashed cells (can_login, groups) in row: {data_row}"
    );
}

#[test]
fn write_whoami_table_renders_fields() {
    let whoami = make_whoami();
    let output = capture_whoami(OutputFormat::Table, &whoami);
    assert!(output.contains("User"));
    assert!(output.contains("testuser"));
    assert!(output.contains("Test User"));
    assert!(output.contains("testuser@example.com"));
    assert!(output.contains("42"));
}

#[test]
fn write_whoami_json_via_write() {
    let whoami = make_whoami();
    let output = capture_whoami(OutputFormat::Json, &whoami);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["name"], "testuser");
    assert_eq!(parsed["real_name"], "Test User");
}

#[test]
fn write_whoami_table_renders_dashes_for_missing_fields() {
    let whoami = WhoamiResponse {
        id: 1,
        name: "bot".into(),
        real_name: None,
        login: None,
    };
    let output = capture_whoami(OutputFormat::Table, &whoami);
    assert!(output.contains("bot"));
    assert!(
        output.contains("Name          -"),
        "expected dashed Name field, got: {output}"
    );
    assert!(
        output.contains("Login         -"),
        "expected dashed Login field, got: {output}"
    );
}
