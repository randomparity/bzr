use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{
    truncate, write_field, write_formatted, write_optional_field, write_table_or_empty, TableSpec,
    DESCRIPTION_TRUNCATE_WIDTH,
};
use crate::types::common::OutputFormat;
use crate::types::component::Component;

const COMPONENT_HEADERS: &[&str] = &["ID", "NAME", "DESCRIPTION", "ASSIGNEE", "ACTIVE"];

fn component_record(c: &Component) -> Vec<String> {
    vec![
        c.id.to_string(),
        c.name.clone(),
        truncate(&c.description, DESCRIPTION_TRUNCATE_WIDTH),
        c.default_assignee
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        if c.is_active { "yes" } else { "no" }.to_string(),
    ]
}

/// Render a flat list of components (id, name, description, assignee,
/// active). JSON output is the full `Component` array.
pub fn write_components<W: Write + ?Sized>(items: &[Component], format: OutputFormat, out: &mut W) {
    write_table_or_empty(
        items,
        format,
        out,
        TableSpec {
            empty_msg: "No components found.",
            headers: COMPONENT_HEADERS,
        },
        component_record,
    );
}

/// Render one component's detail. JSON output is the `Component` object.
pub fn write_component<W: Write + ?Sized>(c: &Component, format: OutputFormat, out: &mut W) {
    write_formatted(c, format, out, |c, out| {
        let _ = writeln!(out, "{} {}", "Component".bold(), c.name.bold());
        write_field(out, "ID", &c.id.to_string());
        if !c.description.is_empty() {
            write_field(out, "Description", &c.description);
        }
        write_optional_field(out, "Default assignee", c.default_assignee.as_deref());
        write_field(out, "Active", if c.is_active { "yes" } else { "no" });
    });
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
