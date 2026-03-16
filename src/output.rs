use colored::Colorize;
use tabled::{Table, Tabled};

use crate::client::{Attachment, Bug, BugzillaUser, Comment, FieldValue, Product};

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!("unknown output format: {other}")),
        }
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
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(bugs).unwrap());
        }
        OutputFormat::Table => {
            if bugs.is_empty() {
                println!("No bugs found.");
                return;
            }
            let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
            let table = Table::new(rows).to_string();
            println!("{table}");
        }
    }
}

pub fn print_bug_detail(bug: &Bug, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(bug).unwrap());
        }
        OutputFormat::Table => {
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
            println!("  Product:     {}", bug.product.as_deref().unwrap_or("-"));
            println!("  Component:   {}", bug.component.as_deref().unwrap_or("-"));
            println!(
                "  Assignee:    {}",
                bug.assigned_to.as_deref().unwrap_or("-")
            );
            println!("  Priority:    {}", bug.priority.as_deref().unwrap_or("-"));
            println!("  Severity:    {}", bug.severity.as_deref().unwrap_or("-"));
            println!("  Creator:     {}", bug.creator.as_deref().unwrap_or("-"));
            println!(
                "  Created:     {}",
                bug.creation_time.as_deref().unwrap_or("-")
            );
            println!(
                "  Updated:     {}",
                bug.last_change_time.as_deref().unwrap_or("-")
            );
            if !bug.keywords.is_empty() {
                println!("  Keywords:    {}", bug.keywords.join(", "));
            }
            if !bug.blocks.is_empty() {
                let ids: Vec<String> = bug
                    .blocks
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                println!("  Blocks:      {}", ids.join(", "));
            }
            if !bug.depends_on.is_empty() {
                let ids: Vec<String> = bug
                    .depends_on
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                println!("  Depends on:  {}", ids.join(", "));
            }
        }
    }
}

pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(attachments).unwrap());
        }
        OutputFormat::Table => {
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
        }
    }
}

pub fn print_comments(comments: &[Comment], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(comments).unwrap());
        }
        OutputFormat::Table => {
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
        }
    }
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
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(products).unwrap());
        }
        OutputFormat::Table => {
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
        }
    }
}

pub fn print_product_detail(product: &Product, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(product).unwrap());
        }
        OutputFormat::Table => {
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
        }
    }
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

pub fn print_field_values(field_name: &str, values: &[FieldValue], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(values).unwrap());
        }
        OutputFormat::Table => {
            if values.is_empty() {
                println!("No values for field '{field_name}'.");
                return;
            }
            let has_transitions = values.iter().any(|v| v.can_change_to.is_some());

            if has_transitions {
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
            } else {
                #[derive(Tabled)]
                struct SimpleFieldRow {
                    #[tabled(rename = "NAME")]
                    name: String,
                    #[tabled(rename = "ACTIVE")]
                    active: String,
                }
                let rows: Vec<SimpleFieldRow> = values
                    .iter()
                    .map(|v| SimpleFieldRow {
                        name: v.name.clone(),
                        active: if v.is_active {
                            "yes".into()
                        } else {
                            "no".into()
                        },
                    })
                    .collect();
                println!("{}", Table::new(rows));
            }
        }
    }
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

pub fn print_users(users: &[BugzillaUser], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(users).unwrap());
        }
        OutputFormat::Table => {
            if users.is_empty() {
                println!("No users found.");
                return;
            }
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
    }
}
