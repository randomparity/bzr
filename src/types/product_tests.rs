#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn product_fields_matches_serialized_keys() {
    let p = Product {
        id: 1,
        name: Some("P".into()),
        description: Some("d".into()),
        is_active: Some(true),
        components: Vec::new(),
        versions: Vec::new(),
        milestones: Vec::new(),
    };
    let value = serde_json::to_value(&p).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> =
        PRODUCT_FIELDS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        serialized, declared,
        "PRODUCT_FIELDS drifted from serde output"
    );
    assert_eq!(
        PRODUCT_FIELDS.len(),
        declared.len(),
        "PRODUCT_FIELDS has duplicates"
    );
}

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
    assert_eq!(product.name.as_deref(), Some("Firefox"));
    assert_eq!(product.is_active, Some(true));
    assert_eq!(product.components.len(), 1);
    assert_eq!(product.components[0].name.as_deref(), Some("General"));
    assert_eq!(
        product.components[0].default_assignee.as_deref(),
        Some("nobody@mozilla.org")
    );
    assert_eq!(product.versions.len(), 1);
    assert_eq!(product.versions[0].name.as_deref(), Some("100.0"));
    assert_eq!(product.milestones.len(), 1);
    assert_eq!(product.milestones[0].name.as_deref(), Some("Future"));
}

#[test]
fn product_deserializes_minimal() {
    let json = serde_json::json!({"id": 5});
    let product: Product = serde_json::from_value(json).unwrap();
    assert_eq!(product.id, 5);
    assert!(product.components.is_empty());
    assert!(product.versions.is_empty());
    assert!(product.milestones.is_empty());

    let serialized = serde_json::to_value(&product).unwrap();
    assert_eq!(serialized["name"], serde_json::Value::Null);
    assert_eq!(serialized["description"], serde_json::Value::Null);
    assert_eq!(serialized["is_active"], serde_json::Value::Null);
}

#[test]
fn version_and_milestone_missing_scalars_serialize_as_null() {
    let version: Version = serde_json::from_value(serde_json::json!({"id": 1})).unwrap();
    let milestone: Milestone = serde_json::from_value(serde_json::json!({"id": 2})).unwrap();

    let version = serde_json::to_value(&version).unwrap();
    let milestone = serde_json::to_value(&milestone).unwrap();
    for value in [&version, &milestone] {
        assert_eq!(value["name"], serde_json::Value::Null);
        assert_eq!(value["sort_key"], serde_json::Value::Null);
        assert_eq!(value["is_active"], serde_json::Value::Null);
    }
}

#[test]
fn version_and_milestone_deserialize() {
    let ver_json = serde_json::json!({"id": 1, "name": "1.0", "sort_key": 5, "is_active": true});
    let ver: Version = serde_json::from_value(ver_json).unwrap();
    assert_eq!(ver.id, 1);
    assert_eq!(ver.sort_key, Some(5));
    assert_eq!(ver.is_active, Some(true));

    let ms_json = serde_json::json!({"id": 2, "name": "M1", "sort_key": 0, "is_active": false});
    let ms: Milestone = serde_json::from_value(ms_json).unwrap();
    assert_eq!(ms.id, 2);
    assert_eq!(ms.is_active, Some(false));
}

#[test]
fn product_metadata_signed_sort_keys_round_trip_supported_domain() {
    for sort_key in [i128::from(i64::MIN), -2008, 0, 2008, i128::from(u64::MAX)] {
        let json = format!(r#"{{"id":1,"name":"1.0","sort_key":{sort_key},"is_active":true}}"#);
        let version: Version = serde_json::from_str(&json).unwrap();
        let milestone: Milestone = serde_json::from_str(&json).unwrap();
        assert_eq!(version.sort_key.unwrap(), sort_key);
        assert_eq!(milestone.sort_key.unwrap(), sort_key);
        for value in [
            serde_json::to_value(&version).unwrap(),
            serde_json::to_value(&milestone).unwrap(),
        ] {
            let serialized_sort_key = value["sort_key"]
                .as_i64()
                .map(i128::from)
                .or_else(|| value["sort_key"].as_u64().map(i128::from));
            assert_eq!(serialized_sort_key, Some(sort_key));
        }
        assert_eq!(version.id, 1_u64);
        assert_eq!(milestone.id, 1_u64);
    }
}

#[test]
fn product_metadata_signed_sort_keys_reject_values_outside_supported_domain() {
    for sort_key in ["-9223372036854775809", "18446744073709551616"] {
        let json = format!(r#"{{"id":1,"name":"1.0","sort_key":{sort_key},"is_active":true}}"#);
        assert!(serde_json::from_str::<Version>(&json).is_err());
        assert!(serde_json::from_str::<Milestone>(&json).is_err());
    }

    for sort_key in [i128::from(i64::MIN) - 1, i128::from(u64::MAX) + 1] {
        let version = Version {
            id: 1,
            name: Some("1.0".into()),
            sort_key: Some(sort_key),
            is_active: Some(true),
        };
        let milestone = Milestone {
            id: 1,
            name: Some("M1".into()),
            sort_key: Some(sort_key),
            is_active: Some(true),
        };
        assert!(serde_json::to_value(version).is_err());
        assert!(serde_json::to_value(milestone).is_err());
    }
}
