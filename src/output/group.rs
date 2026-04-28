use std::io::{self, Write as _};

use colored::Colorize;

use super::formatting::{print_bool_field, print_field, print_formatted};
use crate::types::{GroupInfo, OutputFormat};

pub fn print_group_info(group: &GroupInfo, format: OutputFormat) {
    print_formatted(group, format, |group| {
        let _ = writeln!(io::stdout(), "{} {}", "Group".bold(), group.name.bold());
        print_field("Description", &group.description);
        print_bool_field("Active", group.is_active);
        print_field("ID", &group.id.to_string());
        if !group.membership.is_empty() {
            let _ = writeln!(io::stdout(), "\n{}:", "Members".bold());
            for m in &group.membership {
                let real = m.real_name.as_deref().unwrap_or("");
                let _ = writeln!(io::stdout(), "  {} ({real})", m.name);
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::print_group_info;
    use crate::types::{GroupInfo, GroupMember, OutputFormat};

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
    fn group_text_format_fields() {
        let group = make_group_info();
        assert_eq!(group.name, "core-team");
        assert_eq!(group.description, "Core development team");
        assert!(group.is_active);
        assert_eq!(group.membership.len(), 1);
        assert_eq!(group.membership[0].name, "alice");
        assert_eq!(
            group.membership[0].real_name.as_deref(),
            Some("Alice Smith")
        );
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

    // ── capture_stdout-based formatter tests ─────────────────────────

    #[cfg(unix)]
    #[tokio::test]
    async fn print_group_info_table_renders_all_fields() {
        let _lock = crate::ENV_LOCK.lock().await;
        let group = make_group_info();
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_group_info(&group, OutputFormat::Table);
        })
        .await;
        assert!(output.contains("Group"));
        assert!(output.contains("core-team"));
        assert!(output.contains("Description"));
        assert!(output.contains("Core development team"));
        assert!(output.contains("Active"));
        assert!(output.contains("Yes"));
        assert!(output.contains("ID"));
        assert!(output.contains('5'));
        assert!(output.contains("Members"));
        assert!(output.contains("alice"));
        assert!(output.contains("Alice Smith"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_group_info_table_no_members_omits_section() {
        let _lock = crate::ENV_LOCK.lock().await;
        let group = GroupInfo {
            id: 6,
            name: "empty-group".into(),
            description: "No members".into(),
            is_active: false,
            membership: vec![],
        };
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_group_info(&group, OutputFormat::Table);
        })
        .await;
        assert!(output.contains("empty-group"));
        assert!(output.contains("No"));
        // No members section should be absent
        assert!(!output.contains("Members"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_group_info_table_member_with_no_real_name_renders_empty_parens() {
        let _lock = crate::ENV_LOCK.lock().await;
        let group = GroupInfo {
            id: 7,
            name: "ünicode-group".into(),
            description: "déscription".into(),
            is_active: true,
            membership: vec![GroupMember {
                id: 1,
                name: "héllo".into(),
                real_name: None,
                email: None,
            }],
        };
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_group_info(&group, OutputFormat::Table);
        })
        .await;
        assert!(output.contains("ünicode-group"));
        assert!(output.contains("déscription"));
        assert!(output.contains("héllo"));
        assert!(output.contains("()"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_group_info_json_via_print() {
        let _lock = crate::ENV_LOCK.lock().await;
        let group = make_group_info();
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_group_info(&group, OutputFormat::Json);
        })
        .await;
        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 5);
        assert_eq!(parsed["name"], "core-team");
        assert_eq!(parsed["membership"][0]["name"], "alice");
    }
}
