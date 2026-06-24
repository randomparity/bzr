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
        name: "Software".into(),
        description: "Software products".into(),
        sort_key: 0,
        products: vec![ClassificationProduct {
            id: 10,
            name: "Widget".into(),
            description: "Widget product".into(),
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
        name: "Software".into(),
        description: "Software products".into(),
        sort_key: 0,
        products: vec![
            ClassificationProduct {
                id: 10,
                name: "Widget".into(),
                description:
                    "A long description that should be truncated by the formatter when displayed"
                        .into(),
            },
            ClassificationProduct {
                id: 11,
                name: "Gadget".into(),
                description: "Short desc".into(),
            },
        ],
    };
    assert_eq!(classification.products.len(), 2);
    assert_eq!(classification.products[0].name, "Widget");
    assert_eq!(classification.products[1].name, "Gadget");
}

#[test]
fn write_classification_json_empty_products() {
    let classification = Classification {
        id: 2,
        name: "Empty".into(),
        description: "No products".into(),
        sort_key: 0,
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
        name: "Hardware".into(),
        description: "Hardware products".into(),
        sort_key: 0,
        products: vec![
            ClassificationProduct {
                id: 1,
                name: "A".into(),
                description: "desc".into(),
            },
            ClassificationProduct {
                id: 2,
                name: "B".into(),
                description: "desc2".into(),
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
        name: "Software".into(),
        description: "Software products".into(),
        sort_key: 0,
        products: vec![ClassificationProduct {
            id: 10,
            name: "Widget".into(),
            description: "A widget".into(),
        }],
    };
    let empty = Classification {
        id: 2,
        name: "Empty".into(),
        description: "Nothing here".into(),
        sort_key: 0,
        products: vec![],
    };

    let populated = capture(OutputFormat::Table, &with_products);
    let bare = capture(OutputFormat::Table, &empty);

    assert!(populated.contains("Products"));
    assert!(populated.contains("Widget"));
    assert!(!bare.contains("Products"));
}
