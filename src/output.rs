use colored::Colorize;
use serde::Serialize;
use tabled::{Table, Tabled};

use crate::types::{
    Attachment, Bug, BugzillaUser, Classification, Comment, FieldValue, GroupInfo, HistoryEntry,
    OutputFormat, Product, ServerExtensions, ServerVersion, WhoamiResponse,
};

fn print_json(value: &(impl Serialize + ?Sized)) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serializable to JSON")
    );
}

fn format_or_json<T: Serialize + ?Sized>(
    value: &T,
    format: OutputFormat,
    table_fn: impl FnOnce(&T),
) {
    match format {
        OutputFormat::Json => print_json(value),
        OutputFormat::Table => table_fn(value),
    }
}

pub fn print_result(value: &(impl Serialize + ?Sized), human_message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(value).expect("serializable to JSON")
            );
        }
        OutputFormat::Table => println!("{human_message}"),
    }
}

/// Typed result payload for JSON output of mutation operations.
#[derive(Debug, Serialize)]
pub struct ActionResult {
    pub id: u64,
    pub resource: &'static str,
    pub action: &'static str,
}

fn format_id_list(ids: &[u64]) -> String {
    ids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}

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

fn shorten_email(email: &str) -> String {
    if let Some(at) = email.find('@') {
        email[..at].to_string()
    } else {
        email.to_string()
    }
}

fn colorize_status(status: &str) -> String {
    match status.to_uppercase().as_str() {
        "NEW" | "UNCONFIRMED" => status.green().to_string(),
        "ASSIGNED" | "IN_PROGRESS" => status.yellow().to_string(),
        "RESOLVED" | "VERIFIED" | "CLOSED" => status.red().to_string(),
        _ => status.to_string(),
    }
}

pub fn print_bugs(bugs: &[Bug], format: OutputFormat) {
    format_or_json(bugs, format, |bugs| {
        if bugs.is_empty() {
            println!("No bugs found.");
            return;
        }
        let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
        let table = Table::new(rows).to_string();
        println!("{table}");
    });
}

pub fn print_bug_detail(bug: &Bug, format: OutputFormat) {
    format_or_json(bug, format, |bug| {
        let field = |label: &str, val: Option<&str>| {
            println!("  {label:<12}{}", val.unwrap_or("-"));
        };
        println!(
            "{} #{}\n{}\n",
            "Bug".bold(),
            bug.id.to_string().bold(),
            bug.summary.bold()
        );
        println!("  Status:      {}", colorize_status(&bug.status));
        if let Some(ref r) = bug.resolution {
            if !r.is_empty() {
                println!("  Resolution:  {r}");
            }
        }
        field("Product:    ", bug.product.as_deref());
        field("Component:  ", bug.component.as_deref());
        field("Assignee:   ", bug.assigned_to.as_deref());
        field("Priority:   ", bug.priority.as_deref());
        field("Severity:   ", bug.severity.as_deref());
        field("Creator:    ", bug.creator.as_deref());
        field("Created:    ", bug.creation_time.as_deref());
        field("Updated:    ", bug.last_change_time.as_deref());
        if !bug.keywords.is_empty() {
            println!("  Keywords:    {}", bug.keywords.join(", "));
        }
        if !bug.blocks.is_empty() {
            println!("  Blocks:      {}", format_id_list(&bug.blocks));
        }
        if !bug.depends_on.is_empty() {
            println!("  Depends on:  {}", format_id_list(&bug.depends_on));
        }
    });
}

pub fn print_history(history: &[HistoryEntry], format: OutputFormat) {
    format_or_json(history, format, |history| {
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

pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    format_or_json(attachments, format, |attachments| {
        if attachments.is_empty() {
            println!("No attachments.");
            return;
        }
        for a in attachments {
            let obsolete = if a.is_obsolete { " [OBSOLETE]" } else { "" };
            let private = if a.is_private { " [PRIVATE]" } else { "" };
            println!(
                "{} #{} - {}{}{}",
                "Attachment".bold(),
                a.id,
                a.summary.bold(),
                obsolete.red(),
                private.red(),
            );
            println!(
                "  File:     {} ({}, {} bytes)",
                a.file_name, a.content_type, a.size
            );
            println!("  Creator:  {}", a.creator.as_deref().unwrap_or("-"));
            println!("  Created:  {}", a.creation_time.as_deref().unwrap_or("-"));
            println!();
        }
    });
}

pub fn print_comments(comments: &[Comment], format: OutputFormat) {
    format_or_json(comments, format, |comments| {
        if comments.is_empty() {
            println!("No comments.");
            return;
        }
        for c in comments {
            println!(
                "{} #{} by {} ({})",
                "Comment".bold(),
                c.count,
                c.creator.as_deref().unwrap_or("unknown").cyan(),
                c.creation_time.as_deref().unwrap_or(""),
            );
            if c.is_private {
                println!("  {}", "[PRIVATE]".red());
            }
            println!();
            for line in c.text.lines() {
                println!("  {line}");
            }
            println!();
            println!("{}", "─".repeat(60));
        }
    });
}

#[derive(Tabled)]
struct ProductRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "DESCRIPTION")]
    description: String,
    #[tabled(rename = "COMPONENTS")]
    components: usize,
}

pub fn print_products(products: &[Product], format: OutputFormat) {
    format_or_json(products, format, |products| {
        if products.is_empty() {
            println!("No products found.");
            return;
        }
        let rows: Vec<ProductRow> = products
            .iter()
            .map(|p| {
                let description = truncate(&p.description, 60);
                ProductRow {
                    id: p.id,
                    name: p.name.clone(),
                    description,
                    components: p.components.len(),
                }
            })
            .collect();
        println!("{}", Table::new(rows));
    });
}

pub fn print_product_detail(product: &Product, format: OutputFormat) {
    format_or_json(product, format, |product| {
        println!(
            "{} {}\n{}\n",
            "Product".bold(),
            product.name.bold(),
            product.description
        );
        if !product.components.is_empty() {
            println!("{}:", "Components".bold());
            for c in &product.components {
                let assignee = c.default_assignee.as_deref().unwrap_or("-");
                let active = if c.is_active { "" } else { " [inactive]" };
                println!("  {}{active}  (assignee: {assignee})", c.name);
            }
            println!();
        }
        if !product.versions.is_empty() {
            println!("{}:", "Versions".bold());
            for v in &product.versions {
                let active = if v.is_active { "" } else { " [inactive]" };
                println!("  {}{active}", v.name);
            }
            println!();
        }
        if !product.milestones.is_empty() {
            println!("{}:", "Milestones".bold());
            for m in &product.milestones {
                let active = if m.is_active { "" } else { " [inactive]" };
                println!("  {}{active}", m.name);
            }
        }
    });
}

#[derive(Tabled)]
struct FieldValueRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "ACTIVE")]
    active: String,
    #[tabled(rename = "CAN CHANGE TO")]
    can_change_to: String,
}

pub fn print_field_values(values: &[FieldValue], field_name: &str, format: OutputFormat) {
    format_or_json(values, format, |values| {
        if values.is_empty() {
            println!("No values for field '{field_name}'.");
            return;
        }
        let rows: Vec<FieldValueRow> = values
            .iter()
            .map(|v| {
                let transitions = v
                    .can_change_to
                    .as_ref()
                    .map(|t| {
                        t.iter()
                            .map(|s| s.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                FieldValueRow {
                    name: v.name.clone(),
                    active: if v.is_active {
                        "yes".into()
                    } else {
                        "no".into()
                    },
                    can_change_to: transitions,
                }
            })
            .collect();
        println!("{}", Table::new(rows));
    });
}

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

pub fn print_server_info(
    version: &ServerVersion,
    extensions: &ServerExtensions,
    format: OutputFormat,
) {
    #[derive(Serialize)]
    struct ServerInfoView<'a> {
        version: &'a str,
        extensions: &'a std::collections::HashMap<String, crate::types::ExtensionInfo>,
    }

    let view = ServerInfoView {
        version: &version.version,
        extensions: &extensions.extensions,
    };

    format_or_json(&view, format, |view| {
        println!("{} {}", "Bugzilla version:".bold(), view.version);
        if view.extensions.is_empty() {
            println!("\nNo extensions installed.");
        } else {
            println!("\n{}:", "Extensions".bold());
            for (name, info) in view.extensions {
                let ver = info.version.as_deref().unwrap_or("unknown");
                println!("  {name} ({ver})");
            }
        }
    });
}

pub fn print_classification(classification: &Classification, format: OutputFormat) {
    format_or_json(classification, format, |classification| {
        println!(
            "{} {}\n{}\n",
            "Classification".bold(),
            classification.name.bold(),
            classification.description,
        );
        if !classification.products.is_empty() {
            println!("{}:", "Products".bold());
            for p in &classification.products {
                println!("  {} - {}", p.name, truncate(&p.description, 60));
            }
        }
    });
}

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

#[derive(Serialize)]
struct ServerDisplayInfo {
    url: String,
    email: Option<String>,
    api_key: String,
    auth_method: String,
}

impl ServerDisplayInfo {
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key: mask_api_key(&srv.api_key),
            auth_method: srv
                .auth_method
                .as_ref()
                .map_or_else(|| "auto (not yet detected)".into(), ToString::to_string),
        }
    }
}

#[derive(Serialize)]
struct ConfigView {
    config_file: String,
    default_server: Option<String>,
    servers: std::collections::BTreeMap<String, ServerDisplayInfo>,
}

pub fn print_config(
    config: &crate::config::Config,
    config_path: &std::path::Path,
    format: OutputFormat,
) {
    let servers: std::collections::BTreeMap<String, ServerDisplayInfo> = config
        .servers
        .iter()
        .map(|(name, srv)| (name.clone(), ServerDisplayInfo::from_config(srv)))
        .collect();

    let view = ConfigView {
        config_file: config_path.to_string_lossy().into_owned(),
        default_server: config.default_server.clone(),
        servers,
    };

    format_or_json(&view, format, |v| {
        println!("Config file: {}\n", v.config_file);
        if let Some(ref def) = v.default_server {
            println!("Default server: {def}");
        }
        if v.servers.is_empty() {
            println!("No servers configured.");
        } else {
            for (name, s) in &v.servers {
                println!("\n[{name}]");
                println!("  URL:     {}", s.url);
                if let Some(ref email) = s.email {
                    println!("  Email:   {email}");
                }
                println!("  API Key: {}", s.api_key);
                println!("  Auth:    {}", s.auth_method);
            }
        }
    });
}

fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...", &key[..8])
    } else {
        "***".into()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::useless_vec, clippy::single_char_pattern)]
mod tests {
    use super::*;
    use crate::types::{
        Attachment, Bug, BugzillaUser, Classification, ClassificationProduct, Comment, FieldChange,
        FieldValue, GroupInfo, GroupMember, HistoryEntry, Product, StatusTransition, UserGroup,
        WhoamiResponse,
    };

    // ── Test data helpers ────────────────────────────────────────────

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

    fn make_bug(id: u64, summary: &str, status: &str) -> Bug {
        Bug {
            id,
            summary: summary.into(),
            status: status.into(),
            resolution: None,
            product: Some("TestProduct".into()),
            component: Some("TestComponent".into()),
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
        }
    }

    fn make_comment(count: u64, text: &str) -> Comment {
        Comment {
            id: count + 100,
            bug_id: 42,
            text: text.into(),
            creator: Some("commenter@example.com".into()),
            creation_time: Some("2025-02-01T08:00:00Z".into()),
            count,
            is_private: false,
        }
    }

    fn make_attachment(id: u64, summary: &str) -> Attachment {
        Attachment {
            id,
            bug_id: 42,
            file_name: format!("file_{id}.patch"),
            summary: summary.into(),
            content_type: "text/plain".into(),
            creator: Some("author@example.com".into()),
            creation_time: Some("2025-03-01T09:00:00Z".into()),
            last_change_time: Some("2025-03-02T10:00:00Z".into()),
            size: 1234,
            is_obsolete: false,
            is_private: false,
            data: None,
        }
    }

    fn make_product(id: u64, name: &str) -> Product {
        Product {
            id,
            name: name.into(),
            description: "A test product description".into(),
            is_active: true,
            components: vec![crate::types::Component {
                id: 1,
                name: "General".into(),
                description: "General component".into(),
                is_active: true,
                default_assignee: Some("dev@example.com".into()),
            }],
            versions: vec![crate::types::Version {
                id: 1,
                name: "1.0".into(),
                sort_key: 0,
                is_active: true,
            }],
            milestones: vec![crate::types::Milestone {
                id: 1,
                name: "M1".into(),
                sort_key: 0,
                is_active: true,
            }],
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

    fn make_classification() -> Classification {
        Classification {
            id: 1,
            name: "Software".into(),
            description: "Software products".into(),
            sort_key: 0,
            products: vec![ClassificationProduct {
                id: 10,
                name: "Widget".into(),
                description: "Widget product".into(),
            }],
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

    // ── truncate ─────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let result = truncate("abcdefghij", 7);
        assert_eq!(result, "abcd...");
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn truncate_unicode_counts_chars_not_bytes() {
        // 4 chars, each multi-byte
        let input = "\u{1f600}\u{1f601}\u{1f602}\u{1f603}";
        let result = truncate(input, 4);
        assert_eq!(result, input); // exactly 4 chars, not truncated
        let result = truncate(input, 3);
        assert!(result.ends_with("..."));
    }

    // ── colorize_status ──────────────────────────────────────────────

    #[test]
    fn colorize_status_new_is_green() {
        // colored output includes ANSI codes; just verify it does not panic
        // and produces a non-empty string containing the original status text
        let result = colorize_status("NEW");
        assert!(result.contains("NEW"));
    }

    #[test]
    fn colorize_status_assigned_is_yellow() {
        let result = colorize_status("ASSIGNED");
        assert!(result.contains("ASSIGNED"));
    }

    #[test]
    fn colorize_status_resolved_is_red() {
        let result = colorize_status("RESOLVED");
        assert!(result.contains("RESOLVED"));
    }

    #[test]
    fn colorize_status_unknown_passes_through() {
        let result = colorize_status("CUSTOM");
        assert!(result.contains("CUSTOM"));
    }

    #[test]
    fn colorize_status_case_insensitive() {
        let result = colorize_status("new");
        assert!(result.contains("new"));
    }

    // ── shorten_email ────────────────────────────────────────────────

    #[test]
    fn shorten_email_strips_domain() {
        assert_eq!(shorten_email("alice@example.com"), "alice");
    }

    #[test]
    fn shorten_email_no_at_unchanged() {
        assert_eq!(shorten_email("alice"), "alice");
    }

    #[test]
    fn shorten_email_empty_string() {
        assert_eq!(shorten_email(""), "");
    }

    // ── format_id_list ───────────────────────────────────────────────

    #[test]
    fn format_id_list_empty() {
        assert_eq!(format_id_list(&[]), "");
    }

    #[test]
    fn format_id_list_single() {
        assert_eq!(format_id_list(&[42]), "42");
    }

    #[test]
    fn format_id_list_multiple() {
        assert_eq!(format_id_list(&[1, 2, 3]), "1, 2, 3");
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
        let bugs = vec![make_bug(42, "Login broken", "NEW")];
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

    // ── print_comments ───────────────────────────────────────────────

    #[test]
    fn print_comments_json_empty() {
        let comments: Vec<Comment> = vec![];
        let json = serde_json::to_string_pretty(&comments).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_comments_json_one_comment() {
        let comments = vec![make_comment(0, "First comment text")];
        let json = serde_json::to_string_pretty(&comments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["text"], "First comment text");
        assert_eq!(parsed[0]["count"], 0);
        assert_eq!(parsed[0]["creator"], "commenter@example.com");
    }

    #[test]
    fn print_comments_json_private_flag() {
        let mut comment = make_comment(1, "secret");
        comment.is_private = true;
        let json = serde_json::to_string(&comment).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["is_private"], true);
    }

    // ── print_attachments ────────────────────────────────────────────

    #[test]
    fn print_attachments_json_empty() {
        let attachments: Vec<Attachment> = vec![];
        let json = serde_json::to_string_pretty(&attachments).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_attachments_json_one_attachment() {
        let attachments = vec![make_attachment(10, "Fix patch")];
        let json = serde_json::to_string_pretty(&attachments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["id"], 10);
        assert_eq!(parsed[0]["summary"], "Fix patch");
        assert_eq!(parsed[0]["file_name"], "file_10.patch");
        assert_eq!(parsed[0]["content_type"], "text/plain");
        assert_eq!(parsed[0]["size"], 1234);
    }

    #[test]
    fn print_attachments_json_obsolete_and_private() {
        let mut att = make_attachment(11, "Old patch");
        att.is_obsolete = true;
        att.is_private = true;
        let json = serde_json::to_string(&att).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["is_obsolete"], true);
        assert_eq!(parsed["is_private"], true);
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

    // ── print_products ───────────────────────────────────────────────

    #[test]
    fn print_products_json_empty() {
        let products: Vec<Product> = vec![];
        let json = serde_json::to_string_pretty(&products).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_products_json_one_product() {
        let products = vec![make_product(1, "Widget")];
        let json = serde_json::to_string_pretty(&products).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["name"], "Widget");
        assert!(parsed[0]["components"].is_array());
    }

    #[test]
    fn product_row_conversion() {
        let product = make_product(5, "Gadget");
        let row = ProductRow {
            id: product.id,
            name: product.name.clone(),
            description: truncate(&product.description, 60),
            components: product.components.len(),
        };
        let table = Table::new(vec![row]).to_string();
        assert!(table.contains("5"));
        assert!(table.contains("Gadget"));
        assert!(table.contains("1")); // 1 component
    }

    // ── print_field_values ───────────────────────────────────────────

    #[test]
    fn print_field_values_json_empty() {
        let values: Vec<FieldValue> = vec![];
        let json = serde_json::to_string_pretty(&values).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_field_values_json_with_transitions() {
        let values = vec![FieldValue {
            name: "NEW".into(),
            sort_key: 0,
            is_active: true,
            can_change_to: Some(vec![
                StatusTransition {
                    name: "ASSIGNED".into(),
                },
                StatusTransition {
                    name: "RESOLVED".into(),
                },
            ]),
        }];
        let json = serde_json::to_string_pretty(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["name"], "NEW");
        assert_eq!(parsed[0]["is_active"], true);
        let transitions = parsed[0]["can_change_to"].as_array().unwrap();
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0]["name"], "ASSIGNED");
    }

    #[test]
    fn print_field_values_json_active_and_inactive() {
        let values = vec![
            FieldValue {
                name: "NEW".into(),
                sort_key: 0,
                is_active: true,
                can_change_to: Some(vec![StatusTransition {
                    name: "ASSIGNED".into(),
                }]),
            },
            FieldValue {
                name: "CLOSED".into(),
                sort_key: 1,
                is_active: false,
                can_change_to: None,
            },
        ];
        let json = serde_json::to_string_pretty(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["name"], "NEW");
        assert_eq!(parsed[0]["is_active"], true);
        assert_eq!(parsed[1]["name"], "CLOSED");
        assert_eq!(parsed[1]["is_active"], false);
        assert!(parsed[1]["can_change_to"].is_null());
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

    // ── print_classification ─────────────────────────────────────────

    #[test]
    fn print_classification_json() {
        let classification = make_classification();
        let json = serde_json::to_string_pretty(&classification).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["name"], "Software");
        assert_eq!(parsed["description"], "Software products");
        let products = parsed["products"].as_array().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0]["name"], "Widget");
    }

    #[test]
    fn print_classification_json_empty_products() {
        let classification = Classification {
            id: 2,
            name: "Empty".into(),
            description: "No products".into(),
            sort_key: 0,
            products: vec![],
        };
        let json = serde_json::to_string_pretty(&classification).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["products"].as_array().unwrap().is_empty());
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

    // ── OutputFormat parsing ─────────────────────────────────────────

    #[test]
    fn output_format_from_str() {
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Table
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert!("JSON".parse::<OutputFormat>().is_err());
        assert!("Table".parse::<OutputFormat>().is_err());
        assert!("xml".parse::<OutputFormat>().is_err());
        let err = "XML".parse::<OutputFormat>().unwrap_err();
        assert!(err.contains("expected 'table' or 'json'"));
    }

    #[test]
    fn output_format_default_is_table() {
        assert_eq!(OutputFormat::default(), OutputFormat::Table);
    }

    // ── print_result ─────────────────────────────────────────────────

    #[test]
    fn print_result_json_serializes_value() {
        let value = serde_json::json!({"id": 42});
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"id":42}"#);
    }

    // ── print_server_info ────────────────────────────────────────────

    #[test]
    fn print_server_info_json_combined() {
        let version = ServerVersion {
            version: "5.0.4".into(),
        };
        let extensions = ServerExtensions {
            extensions: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "BmpConvert".into(),
                    crate::types::ExtensionInfo {
                        version: Some("1.0".into()),
                    },
                );
                m
            },
        };
        let combined = serde_json::json!({
            "version": version.version,
            "extensions": extensions.extensions,
        });
        let json = serde_json::to_string_pretty(&combined).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], "5.0.4");
        assert!(parsed["extensions"]["BmpConvert"].is_object());
    }

    // ── print_product_detail ─────────────────────────────────────────

    #[test]
    fn print_product_detail_json() {
        let product = make_product(3, "Acme");
        let json = serde_json::to_string_pretty(&product).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 3);
        assert_eq!(parsed["name"], "Acme");
        assert!(!parsed["components"].as_array().unwrap().is_empty());
        assert!(!parsed["versions"].as_array().unwrap().is_empty());
        assert!(!parsed["milestones"].as_array().unwrap().is_empty());
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

    // ── mask_api_key tests ──────────────────────────────────────────

    #[test]
    fn mask_api_key_long_key_shows_prefix() {
        assert_eq!(mask_api_key("abcdefghijklmnop"), "abcdefgh...");
    }

    #[test]
    fn mask_api_key_short_key_fully_masked() {
        assert_eq!(mask_api_key("short"), "***");
    }

    #[test]
    fn mask_api_key_exactly_8_chars_fully_masked() {
        assert_eq!(mask_api_key("12345678"), "***");
    }

    #[test]
    fn mask_api_key_empty_string_fully_masked() {
        assert_eq!(mask_api_key(""), "***");
    }
}
