use colored::Colorize;
use tabled::{Table, Tabled};

use super::common::{print_colored_field, print_formatted, print_optional_field};
use crate::types::{BugzillaUser, OutputFormat, WhoamiResponse};

#[derive(Tabled)]
struct UserRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "REAL NAME")]
    real_name: String,
    #[tabled(rename = "EMAIL")]
    email: String,
}

#[derive(Tabled)]
struct DetailedUserRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "REAL NAME")]
    real_name: String,
    #[tabled(rename = "EMAIL")]
    email: String,
    #[tabled(rename = "CAN LOGIN")]
    can_login: String,
    #[tabled(rename = "GROUPS")]
    groups: String,
}

fn basic_row(user: &BugzillaUser) -> UserRow {
    UserRow {
        id: user.id,
        name: user.name.clone(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
    }
}

fn detailed_row(user: &BugzillaUser) -> DetailedUserRow {
    DetailedUserRow {
        id: user.id,
        name: user.name.clone(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
        can_login: match user.can_login {
            Some(true) => "Yes".into(),
            Some(false) => "No".into(),
            None => "-".into(),
        },
        groups: if user.groups.is_empty() {
            "-".into()
        } else {
            user.groups
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    }
}

#[expect(clippy::print_stdout)]
pub fn print_users(users: &[BugzillaUser], details: bool, format: OutputFormat) {
    print_formatted(users, format, |users| {
        if users.is_empty() {
            println!("No users found.");
            return;
        }
        if details {
            let rows: Vec<DetailedUserRow> = users.iter().map(detailed_row).collect();
            println!("{}", Table::new(rows));
        } else {
            let rows: Vec<UserRow> = users.iter().map(basic_row).collect();
            println!("{}", Table::new(rows));
        }
    });
}

#[expect(clippy::print_stdout)]
pub fn print_whoami(whoami: &WhoamiResponse, format: OutputFormat) {
    print_formatted(whoami, format, |whoami| {
        println!("{} {}", "User".bold(), whoami.name.bold());
        print_optional_field("Name", whoami.real_name.as_deref());
        print_optional_field("Login", whoami.login.as_deref());
        print_colored_field("ID", &whoami.id.to_string());
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
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
}
