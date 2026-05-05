use std::io::{self, Write as _};

use colored::Colorize;

use super::formatting::{print_formatted, truncate};
use crate::types::{Classification, OutputFormat};

pub fn print_classification(classification: &Classification, format: OutputFormat) {
    print_formatted(classification, format, |classification| {
        let _ = writeln!(
            io::stdout(),
            "{} {}\n{}\n",
            "Classification".bold(),
            classification.name.bold(),
            classification.description,
        );
        if !classification.products.is_empty() {
            let _ = writeln!(io::stdout(), "{}:", "Products".bold());
            for p in &classification.products {
                let _ = writeln!(
                    io::stdout(),
                    "  {} - {}",
                    p.name,
                    truncate(&p.description, 60)
                );
            }
        }
    });
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
