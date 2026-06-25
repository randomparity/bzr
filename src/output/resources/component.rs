use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{
    opt_yes_no, truncate, write_field, write_formatted, write_optional_field, write_table_or_empty,
    TableSpec, DESCRIPTION_TRUNCATE_WIDTH,
};
use crate::types::component::Component;
use crate::types::output::OutputFormat;

const COMPONENT_HEADERS: &[&str] = &["ID", "NAME", "DESCRIPTION", "ASSIGNEE", "ACTIVE"];

fn component_record(c: &Component) -> Vec<String> {
    vec![
        c.id.to_string(),
        c.name.clone().unwrap_or_default(),
        truncate(
            c.description.as_deref().unwrap_or(""),
            DESCRIPTION_TRUNCATE_WIDTH,
        ),
        c.default_assignee
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        opt_yes_no(c.is_active).to_string(),
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
        let _ = writeln!(
            out,
            "{} {}",
            "Component".bold(),
            c.name.as_deref().unwrap_or("unknown").bold()
        );
        write_field(out, "ID", &c.id.to_string());
        if let Some(description) = c.description.as_deref().filter(|value| !value.is_empty()) {
            write_field(out, "Description", description);
        }
        write_optional_field(out, "Default assignee", c.default_assignee.as_deref());
        write_field(out, "Active", opt_yes_no(c.is_active));
    });
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
