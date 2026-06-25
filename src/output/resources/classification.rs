use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{
    truncate, write_formatted, write_table_or_empty, TableSpec, DESCRIPTION_TRUNCATE_WIDTH,
};
use crate::types::classification::Classification;
use crate::types::output::OutputFormat;

const CLASSIFICATION_HEADERS: &[&str] = &["ID", "NAME", "DESCRIPTION", "PRODUCTS"];

fn classification_record(c: &Classification) -> Vec<String> {
    vec![
        c.id.to_string(),
        c.name.clone().unwrap_or_default(),
        truncate(
            c.description.as_deref().unwrap_or(""),
            DESCRIPTION_TRUNCATE_WIDTH,
        ),
        c.products.len().to_string(),
    ]
}

/// Render a flat list of classifications (id, name, description, product
/// count). JSON output is the full `Classification` array.
pub fn write_classifications<W: Write + ?Sized>(
    items: &[Classification],
    format: OutputFormat,
    out: &mut W,
) {
    write_table_or_empty(
        items,
        format,
        out,
        TableSpec {
            empty_msg: "No classifications found.",
            headers: CLASSIFICATION_HEADERS,
        },
        classification_record,
    );
}

pub fn write_classification<W: Write + ?Sized>(
    classification: &Classification,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(classification, format, out, |classification, out| {
        let _ = writeln!(
            out,
            "{} {}\n{}\n",
            "Classification".bold(),
            classification.name.as_deref().unwrap_or("unknown").bold(),
            classification.description.as_deref().unwrap_or("-"),
        );
        if !classification.products.is_empty() {
            let _ = writeln!(out, "{}:", "Products".bold());
            for p in &classification.products {
                let _ = writeln!(
                    out,
                    "  {} - {}",
                    p.name.as_deref().unwrap_or("unknown"),
                    truncate(
                        p.description.as_deref().unwrap_or(""),
                        DESCRIPTION_TRUNCATE_WIDTH
                    )
                );
            }
        }
    });
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
