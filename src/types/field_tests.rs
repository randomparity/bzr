#![expect(clippy::unwrap_used)]

use super::FieldValue;

#[test]
fn field_value_null_name_becomes_empty() {
    let json = r#"{"name": null, "sort_key": 0, "is_active": true}"#;
    let value: FieldValue = serde_json::from_str(json).unwrap();
    assert!(value.name.is_empty());
}

#[test]
fn field_value_with_name() {
    let json = r#"{"name": "RESOLVED", "sort_key": 5, "is_active": true}"#;
    let value: FieldValue = serde_json::from_str(json).unwrap();
    assert_eq!(value.name, "RESOLVED");
    assert_eq!(value.sort_key, 5);
    assert!(value.is_active);
}
