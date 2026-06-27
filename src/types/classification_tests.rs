#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn classification_fields_matches_serialized_keys() {
    let cls = Classification {
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
    let value = serde_json::to_value(&cls).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> = CLASSIFICATION_FIELDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        serialized, declared,
        "CLASSIFICATION_FIELDS drifted from serde output"
    );
    assert_eq!(
        CLASSIFICATION_FIELDS.len(),
        declared.len(),
        "CLASSIFICATION_FIELDS has duplicates"
    );
}

#[test]
fn classification_deserializes() {
    let json = serde_json::json!({
        "id": 2,
        "name": "Client Software",
        "description": "Client apps",
        "sort_key": 10,
        "products": [
            {"id": 1, "name": "Firefox", "description": "Browser"}
        ]
    });
    let cls: Classification = serde_json::from_value(json).unwrap();
    assert_eq!(cls.id, 2);
    assert_eq!(cls.name.as_deref(), Some("Client Software"));
    assert_eq!(cls.sort_key, Some(10));
    assert_eq!(cls.products.len(), 1);
    assert_eq!(cls.products[0].name.as_deref(), Some("Firefox"));
}

#[test]
fn classification_missing_scalars_serialize_as_null() {
    let cls: Classification = serde_json::from_value(serde_json::json!({"id": 2})).unwrap();
    let product: ClassificationProduct =
        serde_json::from_value(serde_json::json!({"id": 1})).unwrap();

    let cls = serde_json::to_value(&cls).unwrap();
    assert_eq!(cls["name"], serde_json::Value::Null);
    assert_eq!(cls["description"], serde_json::Value::Null);
    assert_eq!(cls["sort_key"], serde_json::Value::Null);

    let product = serde_json::to_value(&product).unwrap();
    assert_eq!(product["name"], serde_json::Value::Null);
    assert_eq!(product["description"], serde_json::Value::Null);
}
