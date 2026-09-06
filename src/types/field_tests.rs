#![expect(clippy::unwrap_used)]

use super::{
    FieldName, FieldNameSource, FieldValue, StatusTransition, FIELD_NAME_FIELDS, FIELD_VALUE_FIELDS,
};

#[test]
fn field_name_source_serializes_lowercase() {
    for (source, expected) in [
        (FieldNameSource::Server, "server"),
        (FieldNameSource::Bzr, "bzr"),
        (FieldNameSource::Both, "both"),
    ] {
        // `as_str` is the single definition; serde reaches it through
        // `#[serde(into = "&'static str")]`, so this pins both output modes.
        assert_eq!(source.as_str(), expected);
        let row = FieldName {
            name: "whiteboard".into(),
            source,
        };
        let value = serde_json::to_value(&row).unwrap();
        assert_eq!(value["name"], "whiteboard");
        assert_eq!(value["source"], expected);
    }
}

/// Mirrors `field_value_fields_matches_serialized_keys`. `FIELD_NAME_FIELDS` is
/// the allow-list `projection_for` validates `--fields` against for the
/// no-argument `field list`, so a `FieldName` key added without updating it
/// would make `--fields <newkey>` exit 7 on a key the command emits.
#[test]
fn field_name_fields_matches_serialized_keys() {
    let row = FieldName {
        name: "status_whiteboard".into(),
        source: FieldNameSource::Server,
    };
    let value = serde_json::to_value(&row).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> =
        FIELD_NAME_FIELDS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        serialized, declared,
        "FIELD_NAME_FIELDS drifted from serde output"
    );
    assert_eq!(
        FIELD_NAME_FIELDS.len(),
        declared.len(),
        "FIELD_NAME_FIELDS has duplicates"
    );
}

#[test]
fn field_value_fields_matches_serialized_keys() {
    let value = FieldValue {
        name: Some("NEW".into()),
        sort_key: Some(0),
        is_active: Some(true),
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
    assert_eq!(value.sort_key, Some(5));
    assert_eq!(value.is_active, Some(true));
}

#[test]
fn field_value_missing_scalars_stay_unknown() {
    let value: FieldValue = serde_json::from_str(r#"{"name": "NEW"}"#).unwrap();
    assert_eq!(value.sort_key, None);
    assert_eq!(value.is_active, None);

    let serialized = serde_json::to_value(value).unwrap();
    assert!(serialized["sort_key"].is_null());
    assert!(serialized["is_active"].is_null());
}

#[test]
fn field_value_signed_sort_key_round_trips_supported_domain() {
    for sort_key in [i128::from(i64::MIN), -2008, 0, 2008, i128::from(u64::MAX)] {
        let json = format!(r#"{{"name":"NEW","sort_key":{sort_key},"is_active":true}}"#);
        let value: FieldValue = serde_json::from_str(&json).unwrap();
        assert_eq!(value.sort_key.unwrap(), sort_key);
        let serialized = serde_json::to_value(value).unwrap();
        let serialized_sort_key = serialized["sort_key"]
            .as_i64()
            .map(i128::from)
            .or_else(|| serialized["sort_key"].as_u64().map(i128::from));
        assert_eq!(serialized_sort_key, Some(sort_key));
    }
}

#[test]
fn field_value_signed_sort_key_rejects_values_outside_supported_domain() {
    for sort_key in ["-9223372036854775809", "18446744073709551616"] {
        let json = format!(r#"{{"name":"NEW","sort_key":{sort_key},"is_active":true}}"#);
        assert!(serde_json::from_str::<FieldValue>(&json).is_err());
    }

    for sort_key in [i128::from(i64::MIN) - 1, i128::from(u64::MAX) + 1] {
        let value = FieldValue {
            name: Some("NEW".into()),
            sort_key: Some(sort_key),
            is_active: Some(true),
            can_change_to: None,
        };
        assert!(serde_json::to_value(value).is_err());
    }
}
