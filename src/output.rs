use colored::Colorize;
use tabled::{Table, Tabled};

use crate::client::{Bug, Comment};
use crate::error::Result;

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
            other => Err(format!("unknown output format: {}", other)),
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
        let summary = if b.summary.len() > 72 {
            format!("{}...", &b.summary[..69])
        } else {
            b.summary.clone()
        };
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

pub fn print_bugs(bugs: &[Bug], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(bugs).unwrap());
        }
        OutputFormat::Table => {
            if bugs.is_empty() {
                println!("No bugs found.");
                return Ok(());
            }
            let rows: Vec<BugRow> = bugs.iter().map(BugRow::from).collect();
            let table = Table::new(rows).to_string();
            println!("{}", table);
        }
    }
    Ok(())
}

pub fn print_bug_detail(bug: &Bug, format: &OutputFormat) -> Result<()> {
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
                    println!("  Resolution:  {}", r);
                }
            }
            println!(
                "  Product:     {}",
                bug.product.as_deref().unwrap_or("-")
            );
            println!(
                "  Component:   {}",
                bug.component.as_deref().unwrap_or("-")
            );
            println!(
                "  Assignee:    {}",
                bug.assigned_to.as_deref().unwrap_or("-")
            );
            println!(
                "  Priority:    {}",
                bug.priority.as_deref().unwrap_or("-")
            );
            println!(
                "  Severity:    {}",
                bug.severity.as_deref().unwrap_or("-")
            );
            println!(
                "  Creator:     {}",
                bug.creator.as_deref().unwrap_or("-")
            );
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
                let ids: Vec<String> = bug.blocks.iter().map(|i| i.to_string()).collect();
                println!("  Blocks:      {}", ids.join(", "));
            }
            if !bug.depends_on.is_empty() {
                let ids: Vec<String> = bug.depends_on.iter().map(|i| i.to_string()).collect();
                println!("  Depends on:  {}", ids.join(", "));
            }
        }
    }
    Ok(())
}

pub fn print_comments(comments: &[Comment], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(comments).unwrap());
        }
        OutputFormat::Table => {
            if comments.is_empty() {
                println!("No comments.");
                return Ok(());
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
                    println!("  {}", line);
                }
                println!();
                println!("{}", "─".repeat(60));
            }
        }
    }
    Ok(())
}
