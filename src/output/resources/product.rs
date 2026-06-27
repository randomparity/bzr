use std::fmt::Write as _;
use std::io::Write;

use crate::output::formatting::{
    truncate, write_formatted_projected, write_records_or_empty, TableSpec,
    DESCRIPTION_TRUNCATE_WIDTH,
};
use crate::types::output::OutputFormat;
use crate::types::product::Product;
use crate::validation::fields::FieldProjection;

const PRODUCT_HEADERS: &[&str] = &["ID", "NAME", "DESCRIPTION", "COMPONENTS"];

fn format_named_list(heading: &str, items: &[(impl AsRef<str>, Option<bool>)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut output = format!("{heading}:\n");
    for (name, is_active) in items {
        let active = if *is_active == Some(false) {
            " [inactive]"
        } else {
            ""
        };
        let _ = writeln!(output, "  {}{active}", name.as_ref());
    }
    output.push('\n');
    output
}

fn format_product_detail(product: &Product) -> String {
    let mut output = format!(
        "Product {}\n{}\n\n",
        product.name.as_deref().unwrap_or("unknown"),
        product.description.as_deref().unwrap_or("-"),
    );
    if !product.components.is_empty() {
        output.push_str("Components:\n");
        for c in &product.components {
            let assignee = c.default_assignee.as_deref().unwrap_or("-");
            let active = if c.is_active == Some(false) {
                " [inactive]"
            } else {
                ""
            };
            let _ = writeln!(
                output,
                "  {}{active}  (assignee: {assignee})",
                c.name.as_deref().unwrap_or("unknown")
            );
        }
        output.push('\n');
    }
    let versions: Vec<_> = product
        .versions
        .iter()
        .map(|v| (v.name.as_deref().unwrap_or("unknown"), v.is_active))
        .collect();
    output.push_str(&format_named_list("Versions", &versions));
    let milestones: Vec<_> = product
        .milestones
        .iter()
        .map(|m| (m.name.as_deref().unwrap_or("unknown"), m.is_active))
        .collect();
    output.push_str(&format_named_list("Milestones", &milestones));
    output
}

struct ProductRow {
    id: u64,
    name: String,
    description: String,
    components: usize,
}

fn product_row(p: &Product) -> ProductRow {
    ProductRow {
        id: p.id,
        name: p.name.clone().unwrap_or_default(),
        description: truncate(
            p.description.as_deref().unwrap_or(""),
            DESCRIPTION_TRUNCATE_WIDTH,
        ),
        components: p.components.len(),
    }
}

fn product_record(product: &Product) -> Vec<String> {
    let row = product_row(product);
    vec![
        row.id.to_string(),
        row.name,
        row.description,
        row.components.to_string(),
    ]
}

pub fn write_products<W: Write + ?Sized>(
    products: &[Product],
    format: OutputFormat,
    projection: &FieldProjection,
    out: &mut W,
) {
    write_formatted_projected(products, format, projection, out, |products, out| {
        write_records_or_empty(
            products,
            TableSpec {
                empty_msg: "No products found.",
                headers: PRODUCT_HEADERS,
            },
            product_record,
            out,
        );
    });
}

pub fn write_product_detail<W: Write + ?Sized>(
    product: &Product,
    format: OutputFormat,
    projection: &FieldProjection,
    out: &mut W,
) {
    write_formatted_projected(product, format, projection, out, |product, out| {
        let _ = write!(out, "{}", format_product_detail(product));
    });
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
