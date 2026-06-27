#![expect(clippy::unwrap_used)]

use super::{FieldValue, StatusTransition, FIELD_VALUE_FIELDS};

#[test]
fn field_value_fields_matches_serialized_keys() {
    let value = FieldValue {
        name: Some("NEW".into()),
        sort_key: 0,
        is_active: true,
        can_change_to: Some(vec![StatusTransition {
            name: "ASSIGNED".into(),
        }]),
    };
    let value = serde_json::to_value(&value).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> = FIELD_VALUE_FIELDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        serialized, declared,
        "FIELD_VALUE_FIELDS drifted from serde output"
    );
    assert_eq!(
        FIELD_VALUE_FIELDS.len(),
        declared.len(),
        "FIELD_VALUE_FIELDS has duplicates"
    );
}

#[test]
fn field_value_null_name_stays_none() {
    let json = r#"{"name": null, "sort_key": 0, "is_active": true}"#;
    let value: FieldValue = serde_json::from_str(json).unwrap();
    assert_eq!(value.name, None);
}

#[test]
fn field_value_null_name_serializes_as_null() {
    let json = r#"{"name": null, "sort_key": 0, "is_active": true}"#;
    let value: FieldValue = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(value).unwrap();

    assert!(serialized["name"].is_null());
}

#[test]
fn field_value_with_name() {
    let json = r#"{"name": "RESOLVED", "sort_key": 5, "is_active": true}"#;
    let value: FieldValue = serde_json::from_str(json).unwrap();
    assert_eq!(value.name.as_deref(), Some("RESOLVED"));
    assert_eq!(value.sort_key, 5);
    assert!(value.is_active);
}
