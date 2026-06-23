#![expect(clippy::unwrap_used)]

use super::FieldValue;

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
