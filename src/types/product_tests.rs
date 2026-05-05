#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn product_deserializes_full() {
    let json = serde_json::json!({
        "id": 1,
        "name": "Firefox",
        "description": "The Firefox browser",
        "is_active": true,
        "components": [
            {
                "id": 10,
                "name": "General",
                "description": "General issues",
                "is_active": true,
                "default_assignee": "nobody@mozilla.org"
            }
        ],
        "versions": [
            {
                "id": 20,
                "name": "100.0",
                "sort_key": 100,
                "is_active": true
            }
        ],
        "milestones": [
            {
                "id": 30,
                "name": "Future",
                "sort_key": 0,
                "is_active": true
            }
        ]
    });
    let product: Product = serde_json::from_value(json).unwrap();
    assert_eq!(product.id, 1);
    assert_eq!(product.name, "Firefox");
    assert!(product.is_active);
    assert_eq!(product.components.len(), 1);
    assert_eq!(product.components[0].name, "General");
    assert_eq!(
        product.components[0].default_assignee.as_deref(),
        Some("nobody@mozilla.org")
    );
    assert_eq!(product.versions.len(), 1);
    assert_eq!(product.versions[0].name, "100.0");
    assert_eq!(product.milestones.len(), 1);
    assert_eq!(product.milestones[0].name, "Future");
}

#[test]
fn product_deserializes_minimal() {
    let json = serde_json::json!({"id": 5});
    let product: Product = serde_json::from_value(json).unwrap();
    assert_eq!(product.id, 5);
    assert_eq!(product.name, "");
    assert!(!product.is_active);
    assert!(product.components.is_empty());
    assert!(product.versions.is_empty());
    assert!(product.milestones.is_empty());
}

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
    assert_eq!(cls.name, "Client Software");
    assert_eq!(cls.sort_key, 10);
    assert_eq!(cls.products.len(), 1);
    assert_eq!(cls.products[0].name, "Firefox");
}

#[test]
fn version_and_milestone_deserialize() {
    let ver_json = serde_json::json!({"id": 1, "name": "1.0", "sort_key": 5, "is_active": true});
    let ver: Version = serde_json::from_value(ver_json).unwrap();
    assert_eq!(ver.id, 1);
    assert_eq!(ver.sort_key, 5);
    assert!(ver.is_active);

    let ms_json = serde_json::json!({"id": 2, "name": "M1", "sort_key": 0, "is_active": false});
    let ms: Milestone = serde_json::from_value(ms_json).unwrap();
    assert_eq!(ms.id, 2);
    assert!(!ms.is_active);
}
