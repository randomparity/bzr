#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn component_deserializes_without_assignee() {
    let json = serde_json::json!({
        "id": 11,
        "name": "UI",
        "description": "User interface",
        "is_active": false
    });
    let component: Component = serde_json::from_value(json).unwrap();
    assert_eq!(component.id, 11);
    assert_eq!(component.name.as_deref(), Some("UI"));
    assert_eq!(component.is_active, Some(false));
    assert!(component.default_assignee.is_none());
}

#[test]
fn component_missing_scalars_serialize_as_null() {
    let component: Component = serde_json::from_value(serde_json::json!({"id": 11})).unwrap();

    let serialized = serde_json::to_value(&component).unwrap();
    assert_eq!(serialized["name"], serde_json::Value::Null);
    assert_eq!(serialized["description"], serde_json::Value::Null);
    assert_eq!(serialized["is_active"], serde_json::Value::Null);
}
