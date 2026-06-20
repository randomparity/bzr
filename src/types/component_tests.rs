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
    assert_eq!(component.name, "UI");
    assert!(!component.is_active);
    assert!(component.default_assignee.is_none());
}
