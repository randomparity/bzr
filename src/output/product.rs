use colored::Colorize;
use tabled::{Table, Tabled};

use super::common::{print_formatted, truncate};
use crate::types::{OutputFormat, Product};

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

#[expect(clippy::print_stdout)]
pub fn print_products(products: &[Product], format: OutputFormat) {
    print_formatted(products, format, |products| {
        if products.is_empty() {
            println!("No products found.");
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
        println!("{}", Table::new(rows));
    });
}

fn print_named_list(heading: &str, items: &[(impl AsRef<str>, bool)]) {
    if items.is_empty() {
        return;
    }
    println!("{}:", heading.bold());
    for (name, is_active) in items {
        let active = if *is_active { "" } else { " [inactive]" };
        println!("  {}{active}", name.as_ref());
    }
    println!();
}

#[expect(clippy::print_stdout)]
pub fn print_product_detail(product: &Product, format: OutputFormat) {
    print_formatted(product, format, |product| {
        println!(
            "{} {}\n{}\n",
            "Product".bold(),
            product.name.bold(),
            product.description
        );
        if !product.components.is_empty() {
            println!("{}:", "Components".bold());
            for c in &product.components {
                let assignee = c.default_assignee.as_deref().unwrap_or("-");
                let active = if c.is_active { "" } else { " [inactive]" };
                println!("  {}{active}  (assignee: {assignee})", c.name);
            }
            println!();
        }
        let versions: Vec<_> = product
            .versions
            .iter()
            .map(|v| (v.name.as_str(), v.is_active))
            .collect();
        print_named_list("Versions", &versions);
        let milestones: Vec<_> = product
            .milestones
            .iter()
            .map(|m| (m.name.as_str(), m.is_active))
            .collect();
        print_named_list("Milestones", &milestones);
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::Product;
    use tabled::Table;

    fn make_product(id: u64, name: &str) -> Product {
        Product {
            id,
            name: name.into(),
            description: "A test product description".into(),
            is_active: true,
            components: vec![crate::types::Component {
                id: 1,
                name: "General".into(),
                description: "General component".into(),
                is_active: true,
                default_assignee: Some("dev@example.com".into()),
            }],
            versions: vec![crate::types::Version {
                id: 1,
                name: "1.0".into(),
                sort_key: 0,
                is_active: true,
            }],
            milestones: vec![crate::types::Milestone {
                id: 1,
                name: "M1".into(),
                sort_key: 0,
                is_active: true,
            }],
        }
    }

    // ── print_products ───────────────────────────────────────────────

    #[test]
    fn print_products_json_empty() {
        let products: Vec<Product> = vec![];
        let json = serde_json::to_string_pretty(&products).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_products_json_one_product() {
        let products = vec![make_product(1, "Widget")];
        let json = serde_json::to_string_pretty(&products).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["name"], "Widget");
        assert!(parsed[0]["components"].is_array());
    }

    #[test]
    fn product_row_conversion() {
        let product = make_product(5, "Gadget");
        let row = ProductRow {
            id: product.id,
            name: product.name.clone(),
            description: truncate(&product.description, 60),
            components: product.components.len(),
        };
        let table = Table::new(vec![row]).to_string();
        assert!(table.contains('5'));
        assert!(table.contains("Gadget"));
        assert!(table.contains('1')); // 1 component
    }

    // ── print_product_detail ─────────────────────────────────────────

    #[test]
    fn print_product_detail_json() {
        let product = make_product(3, "Acme");
        let json = serde_json::to_string_pretty(&product).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 3);
        assert_eq!(parsed["name"], "Acme");
        assert!(!parsed["components"].as_array().unwrap().is_empty());
        assert!(!parsed["versions"].as_array().unwrap().is_empty());
        assert!(!parsed["milestones"].as_array().unwrap().is_empty());
    }

}
