#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::Product;

fn make_product(id: u64, name: &str) -> Product {
    Product {
        id,
        name: Some(name.into()),
        description: Some("A test product description".into()),
        is_active: Some(true),
        components: vec![crate::types::Component {
            id: 1,
            name: Some("General".into()),
            description: Some("General component".into()),
            is_active: Some(true),
            default_assignee: Some("dev@example.com".into()),
        }],
        versions: vec![crate::types::Version {
            id: 1,
            name: Some("1.0".into()),
            sort_key: Some(0),
            is_active: Some(true),
        }],
        milestones: vec![crate::types::Milestone {
            id: 1,
            name: Some("M1".into()),
            sort_key: Some(0),
            is_active: Some(true),
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
    let row = product_row(&product);
    assert_eq!(row.id, 5);
    assert_eq!(row.name, "Gadget");
    assert_eq!(
        row.description,
        truncate(product.description.as_deref().unwrap_or(""), 60)
    );
    assert_eq!(row.components, 1);
}

// ── product_record ───────────────────────────────────────────────

#[test]
fn product_record_has_four_columns_with_correct_values() {
    // Kill vec![]/vec![String::new()]/vec!["xyzzy".into()] mutants on
    // `product_record`: assert every column position carries the real value.
    let mut product = make_product(42, "Firefox");
    product.description = Some("The browser".into());
    // Add a second component so the count is >1 and can't match an empty vec.
    product.components.push(crate::types::Component {
        id: 2,
        name: Some("Networking".into()),
        description: Some("Networking component".into()),
        is_active: Some(true),
        default_assignee: None,
    });
    let row = product_record(&product);
    assert_eq!(row.len(), 4, "record must have exactly 4 columns");
    assert_eq!(row[0], "42", "column 0 must be the product id");
    assert_eq!(row[1], "Firefox", "column 1 must be the product name");
    assert!(
        row[2].contains("The browser"),
        "column 2 must contain the description"
    );
    assert_eq!(row[3], "2", "column 3 must be the component count");
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
    let empty: [(&str, Option<bool>); 0] = [];
    assert!(format_named_list("Versions", &empty).is_empty());
}

#[test]
fn format_product_detail_renders_sections_and_inactive_flags() {
    let mut product = make_product(3, "Acme");
    product.components.push(crate::types::Component {
        id: 2,
        name: Some("Inactive".into()),
        description: Some("Inactive component".into()),
        is_active: Some(false),
        default_assignee: None,
    });
    product.versions.push(crate::types::Version {
        id: 2,
        name: Some("2.0".into()),
        sort_key: Some(1),
        is_active: Some(false),
    });
    product.milestones.push(crate::types::Milestone {
        id: 2,
        name: Some("M2".into()),
        sort_key: Some(1),
        is_active: Some(false),
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
