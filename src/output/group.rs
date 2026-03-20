use colored::Colorize;

use super::common::{print_bool_field, print_colored_field, print_formatted};
use crate::types::{GroupInfo, OutputFormat};

#[expect(clippy::print_stdout)]
pub fn print_group_info(group: &GroupInfo, format: OutputFormat) {
    print_formatted(group, format, |group| {
        println!("{} {}", "Group".bold(), group.name.bold());
        print_colored_field("Description", &group.description);
        print_bool_field("Active", group.is_active);
        print_colored_field("ID", &group.id.to_string());
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
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::{GroupInfo, GroupMember};

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
}
