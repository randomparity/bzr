use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{truncate, write_formatted, DESCRIPTION_TRUNCATE_WIDTH};
use crate::types::{Classification, OutputFormat};

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
            classification.name.bold(),
            classification.description,
        );
        if !classification.products.is_empty() {
            let _ = writeln!(out, "{}:", "Products".bold());
            for p in &classification.products {
                let _ = writeln!(
                    out,
                    "  {} - {}",
                    p.name,
                    truncate(&p.description, DESCRIPTION_TRUNCATE_WIDTH)
                );
            }
        }
    });
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
