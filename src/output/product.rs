use std::fmt::Write as _;
use std::io::Write;

use tabled::{Table, Tabled};

use super::formatting::{truncate, write_formatted};
use crate::types::{OutputFormat, Product};

fn format_named_list(heading: &str, items: &[(impl AsRef<str>, bool)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut output = format!("{heading}:\n");
    for (name, is_active) in items {
        let active = if *is_active { "" } else { " [inactive]" };
        let _ = writeln!(output, "  {}{active}", name.as_ref());
    }
    output.push('\n');
    output
}

fn format_product_detail(product: &Product) -> String {
    let mut output = format!("Product {}\n{}\n\n", product.name, product.description);
    if !product.components.is_empty() {
        output.push_str("Components:\n");
        for c in &product.components {
            let assignee = c.default_assignee.as_deref().unwrap_or("-");
            let active = if c.is_active { "" } else { " [inactive]" };
            let _ = writeln!(output, "  {}{active}  (assignee: {assignee})", c.name);
        }
        output.push('\n');
    }
    let versions: Vec<_> = product
        .versions
        .iter()
        .map(|v| (v.name.as_str(), v.is_active))
        .collect();
    output.push_str(&format_named_list("Versions", &versions));
    let milestones: Vec<_> = product
        .milestones
        .iter()
        .map(|m| (m.name.as_str(), m.is_active))
        .collect();
    output.push_str(&format_named_list("Milestones", &milestones));
    output
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

pub fn write_products<W: Write + ?Sized>(products: &[Product], format: OutputFormat, out: &mut W) {
    write_formatted(products, format, out, |products, out| {
        if products.is_empty() {
            let _ = writeln!(out, "No products found.");
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
        let _ = writeln!(out, "{}", Table::new(rows));
    });
}

pub fn write_product_detail<W: Write + ?Sized>(
    product: &Product,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(product, format, out, |product, out| {
        let _ = write!(out, "{}", format_product_detail(product));
    });
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
