use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{opt_yes_no, write_field, write_formatted, write_optional_field};
use crate::types::group::GroupInfo;
use crate::types::output::OutputFormat;

pub fn write_group_info<W: Write + ?Sized>(group: &GroupInfo, format: OutputFormat, out: &mut W) {
    write_formatted(group, format, out, |group, out| {
        let _ = writeln!(
            out,
            "{} {}",
            "Group".bold(),
            group.name.as_deref().unwrap_or("unknown").bold()
        );
        write_optional_field(out, "Description", group.description.as_deref());
        write_field(out, "Active", opt_yes_no(group.is_active));
        write_field(out, "ID", &group.id.to_string());
        if !group.membership.is_empty() {
            let _ = writeln!(out, "\n{}:", "Members".bold());
            for m in &group.membership {
                let real = m.real_name.as_deref().unwrap_or("");
                let _ = writeln!(out, "  {} ({real})", m.name.as_deref().unwrap_or("unknown"));
            }
        }
    });
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
