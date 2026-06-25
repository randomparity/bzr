#![expect(clippy::unwrap_used)]

use super::write_classification;
use crate::types::{Classification, ClassificationProduct, OutputFormat};

fn capture(format: OutputFormat, c: &Classification) -> String {
    let mut buf = Vec::new();
    write_classification(c, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_classification_json() {
    let classification = Classification {
        id: 1,
        name: Some("Software".into()),
        description: Some("Software products".into()),
        sort_key: Some(0),
        products: vec![ClassificationProduct {
            id: 10,
            name: Some("Widget".into()),
            description: Some("Widget product".into()),
        }],
    };
    let json = serde_json::to_string(&classification).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["name"], "Software");
    assert_eq!(parsed["products"][0]["name"], "Widget");
}

#[test]
fn classification_text_format_fields() {
    let classification = Classification {
        id: 1,
        name: Some("Software".into()),
        description: Some("Software products".into()),
        sort_key: Some(0),
        products: vec![
            ClassificationProduct {
                id: 10,
                name: Some("Widget".into()),
                description: Some(
                    "A long description that should be truncated by the formatter when displayed"
                        .into(),
                ),
            },
            ClassificationProduct {
                id: 11,
                name: Some("Gadget".into()),
                description: Some("Short desc".into()),
            },
        ],
    };
    assert_eq!(classification.products.len(), 2);
    assert_eq!(classification.products[0].name.as_deref(), Some("Widget"));
    assert_eq!(classification.products[1].name.as_deref(), Some("Gadget"));
}

#[test]
fn write_classification_json_empty_products() {
    let classification = Classification {
        id: 2,
        name: Some("Empty".into()),
        description: Some("No products".into()),
        sort_key: Some(0),
        products: vec![],
    };
    let json = serde_json::to_string(&classification).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["products"].as_array().unwrap().len(), 0);
}

// ── classification_record ────────────────────────────────────────

#[test]
fn classification_record_has_four_columns_with_correct_values() {
    // Kill vec![]/vec![String::new()]/vec!["xyzzy".into()] mutants on
    // `classification_record`: assert every column position carries the real value.
    let c = Classification {
        id: 7,
        name: Some("Hardware".into()),
        description: Some("Hardware products".into()),
        sort_key: Some(0),
        products: vec![
            ClassificationProduct {
                id: 1,
                name: Some("A".into()),
                description: Some("desc".into()),
            },
            ClassificationProduct {
                id: 2,
                name: Some("B".into()),
                description: Some("desc2".into()),
            },
        ],
    };
    let row = super::classification_record(&c);
    assert_eq!(row.len(), 4, "record must have exactly 4 columns");
    assert_eq!(row[0], "7", "column 0 must be the classification id");
    assert_eq!(
        row[1], "Hardware",
        "column 1 must be the classification name"
    );
    assert!(
        row[2].contains("Hardware products"),
        "column 2 must contain the description"
    );
    assert_eq!(row[3], "2", "column 3 must be the product count");
}

#[test]
fn write_classification_table_omits_products_header_when_empty() {
    let with_products = Classification {
        id: 1,
        name: Some("Software".into()),
        description: Some("Software products".into()),
        sort_key: Some(0),
        products: vec![ClassificationProduct {
            id: 10,
            name: Some("Widget".into()),
            description: Some("A widget".into()),
        }],
    };
    let empty = Classification {
        id: 2,
        name: Some("Empty".into()),
        description: Some("Nothing here".into()),
        sort_key: Some(0),
        products: vec![],
    };

    let populated = capture(OutputFormat::Table, &with_products);
    let bare = capture(OutputFormat::Table, &empty);

    assert!(populated.contains("Products"));
    assert!(populated.contains("Widget"));
    assert!(!bare.contains("Products"));
}
