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
#[path = "group_tests.rs"]
mod tests;
