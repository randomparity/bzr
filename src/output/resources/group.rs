use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{write_bool_field, write_field, write_formatted};
use crate::types::{GroupInfo, OutputFormat};

pub fn write_group_info<W: Write + ?Sized>(group: &GroupInfo, format: OutputFormat, out: &mut W) {
    write_formatted(group, format, out, |group, out| {
        let _ = writeln!(out, "{} {}", "Group".bold(), group.name.bold());
        write_field(out, "Description", &group.description);
        write_bool_field(out, "Active", group.is_active);
        write_field(out, "ID", &group.id.to_string());
        if !group.membership.is_empty() {
            let _ = writeln!(out, "\n{}:", "Members".bold());
            for m in &group.membership {
                let real = m.real_name.as_deref().unwrap_or("");
                let _ = writeln!(out, "  {} ({real})", m.name);
            }
        }
    });
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
