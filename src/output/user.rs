use colored::Colorize;
use tabled::{Table, Tabled};

use super::format_or_json;
use crate::types::{BugzillaUser, GroupInfo, OutputFormat, WhoamiResponse};

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
    format_or_json(users, format, |users| {
        if users.is_empty() {
            println!("No users found.");
            return;
        }
        if details {
            let rows: Vec<DetailedUserRow> = users.iter().map(detailed_row).collect();
            println!("{}", Table::new(rows));
        } else {
            let rows: Vec<UserRow> = users
                .iter()
                .map(|u| UserRow {
                    id: u.id,
                    name: u.name.clone(),
                    real_name: u.real_name.clone().unwrap_or_default(),
                    email: u.email.clone().unwrap_or_default(),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
    });
}

#[expect(clippy::print_stdout)]
pub fn print_whoami(whoami: &WhoamiResponse, format: OutputFormat) {
    format_or_json(whoami, format, |whoami| {
        println!("{} {}", "User".bold(), whoami.name.bold());
        if let Some(ref real_name) = whoami.real_name {
            println!("  Name:   {real_name}");
        }
        if let Some(ref login) = whoami.login {
            println!("  Login:  {login}");
        }
        println!("  ID:     {}", whoami.id);
    });
}

#[expect(clippy::print_stdout)]
pub fn print_group_info(group: &GroupInfo, format: OutputFormat) {
    format_or_json(group, format, |group| {
        println!("{} {}", "Group".bold(), group.name.bold());
        println!("  Description:  {}", group.description);
        println!(
            "  Active:       {}",
            if group.is_active { "yes" } else { "no" }
        );
        println!("  ID:           {}", group.id);
        if !group.membership.is_empty() {
            println!("\n{}:", "Members".bold());
            for m in &group.membership {
                let real = m.real_name.as_deref().unwrap_or("");
                println!("  {} ({real})", m.name);
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::useless_vec, clippy::single_char_pattern)]
mod tests {
    use super::*;
    use crate::types::{BugzillaUser, GroupInfo, GroupMember, UserGroup, WhoamiResponse};
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

    fn make_group_info() -> GroupInfo {
        GroupInfo {
            id: 5,
            name: "core-team".into(),
            description: "Core development team".into(),
            is_active: true,
            membership: vec![GroupMember {
                id: 1,
                name: "alice".into(),
                real_name: Some("Alice Smith".into()),
                email: Some("alice@example.com".into()),
            }],
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

    // ── print_group_info ─────────────────────────────────────────────

    #[test]
    fn print_group_info_json() {
        let group = make_group_info();
        let json = serde_json::to_string_pretty(&group).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 5);
        assert_eq!(parsed["name"], "core-team");
        assert_eq!(parsed["description"], "Core development team");
        assert_eq!(parsed["is_active"], true);
        let members = parsed["membership"].as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["name"], "alice");
        assert_eq!(members[0]["real_name"], "Alice Smith");
    }

    #[test]
    fn print_group_info_json_no_members() {
        let group = GroupInfo {
            id: 6,
            name: "empty-group".into(),
            description: "No members".into(),
            is_active: false,
            membership: vec![],
        };
        let json = serde_json::to_string_pretty(&group).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["is_active"], false);
        assert!(parsed["membership"].as_array().unwrap().is_empty());
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
