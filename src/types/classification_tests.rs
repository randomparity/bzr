#![expect(clippy::unwrap_used)]

use super::*;

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
