#![expect(clippy::unwrap_used)]

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

// ── write_products ───────────────────────────────────────────────

#[test]
fn write_products_json_empty() {
    let products: Vec<Product> = vec![];
    let json = serde_json::to_string_pretty(&products).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_products_json_one_product() {
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

// ── write_product_detail ─────────────────────────────────────────

#[test]
fn write_product_detail_json() {
    let product = make_product(3, "Acme");
    let json = serde_json::to_string_pretty(&product).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 3);
    assert_eq!(parsed["name"], "Acme");
    assert!(!parsed["components"].as_array().unwrap().is_empty());
    assert!(!parsed["versions"].as_array().unwrap().is_empty());
    assert!(!parsed["milestones"].as_array().unwrap().is_empty());
}

#[test]
fn format_named_list_empty_is_blank() {
    let empty: [(&str, bool); 0] = [];
    assert!(format_named_list("Versions", &empty).is_empty());
}

#[test]
fn format_product_detail_renders_sections_and_inactive_flags() {
    let mut product = make_product(3, "Acme");
    product.components.push(crate::types::Component {
        id: 2,
        name: "Inactive".into(),
        description: "Inactive component".into(),
        is_active: false,
        default_assignee: None,
    });
    product.versions.push(crate::types::Version {
        id: 2,
        name: "2.0".into(),
        sort_key: 1,
        is_active: false,
    });
    product.milestones.push(crate::types::Milestone {
        id: 2,
        name: "M2".into(),
        sort_key: 1,
        is_active: false,
    });

    let output = format_product_detail(&product);

    assert!(output.contains("Product"));
    assert!(output.contains("Acme"));
    assert!(output.contains("Components:"));
    assert!(output.contains("General  (assignee: dev@example.com)"));
    assert!(output.contains("Inactive [inactive]  (assignee: -)"));
    assert!(output.contains("Versions:"));
    assert!(output.contains("1.0"));
    assert!(output.contains("2.0 [inactive]"));
    assert!(output.contains("Milestones:"));
    assert!(output.contains("M1"));
    assert!(output.contains("M2 [inactive]"));
}
