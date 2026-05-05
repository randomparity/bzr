#![expect(clippy::unwrap_used)]

use crate::types::{Classification, ClassificationProduct};

#[test]
fn print_classification_json() {
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
fn print_classification_json_empty_products() {
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

#[cfg(unix)]
#[tokio::test]
async fn print_classification_table_omits_products_header_when_empty() {
    let _lock = crate::ENV_LOCK.lock().await;
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

    let ((), populated) = crate::test_helpers::capture_stdout(async {
        super::print_classification(&with_products, crate::types::OutputFormat::Table);
    })
    .await;
    let ((), bare) = crate::test_helpers::capture_stdout(async {
        super::print_classification(&empty, crate::types::OutputFormat::Table);
    })
    .await;

    assert!(populated.contains("Products"));
    assert!(populated.contains("Widget"));
    assert!(!bare.contains("Products"));
}
