#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn bug_deserializes_minimal() {
    let json = r#"{"id": 42}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.id, 42);
    assert!(bug.keywords.is_empty());
    assert!(bug.custom_fields.is_empty());

    let serialized = serde_json::to_value(&bug).unwrap();
    assert_eq!(serialized["summary"], serde_json::Value::Null);
    assert_eq!(serialized["status"], serde_json::Value::Null);
}

#[test]
fn bug_deserializes_full() {
    let json = r#"{
        "id": 1,
        "summary": "test bug",
        "status": "NEW",
        "product": "Core",
        "component": "General",
        "priority": "P1",
        "keywords": ["regression"]
    }"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.summary.as_deref(), Some("test bug"));
    assert_eq!(bug.status.as_deref(), Some("NEW"));
    assert_eq!(bug.product.as_deref(), Some("Core"));
    assert_eq!(bug.keywords, vec!["regression"]);
}

#[test]
fn bug_deserializes_deadline() {
    let json = r#"{"id": 42, "deadline": "2026-12-31"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["deadline"], "2026-12-31");
}

#[test]
fn bug_deserializes_target_milestone_and_flags() {
    let json = r#"{
        "id": 42,
        "target_milestone": "9.0",
        "flags": [
            {"name": "review", "status": "+", "setter": "alice@example.com"},
            {"name": "needinfo", "status": "?", "requestee": "bob@example.com"}
        ]
    }"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.target_milestone.as_deref(), Some("9.0"));
    assert_eq!(bug.flags.len(), 2);
    assert_eq!(bug.flags[0].name.as_deref(), Some("review"));
    assert_eq!(bug.flags[0].status.as_deref(), Some("+"));
    assert_eq!(bug.flags[0].setter.as_deref(), Some("alice@example.com"));
    assert_eq!(bug.flags[1].status.as_deref(), Some("?"));
    assert_eq!(bug.flags[1].requestee.as_deref(), Some("bob@example.com"));
}

#[test]
fn bug_without_flags_defaults_to_empty_and_serializes_array() {
    let bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    assert!(bug.flags.is_empty());
    assert!(bug.target_milestone.is_none());

    let serialized = serde_json::to_value(&bug).unwrap();
    // flags is always present as an array (empty -> []), target_milestone null.
    assert_eq!(serialized["flags"], serde_json::json!([]));
    assert!(serialized["target_milestone"].is_null());
}

#[test]
fn bug_serializes_flags_and_target_milestone() {
    let json =
        r#"{"id": 7, "target_milestone": "---", "flags": [{"name": "review", "status": "+"}]}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&bug).unwrap();

    // JSON stays faithful: the raw "---" sentinel is preserved (only the table
    // detail suppresses it).
    assert_eq!(serialized["target_milestone"], "---");
    assert_eq!(serialized["flags"][0]["name"], "review");
    assert_eq!(serialized["flags"][0]["status"], "+");
}

#[test]
fn flag_with_unexpected_status_token_still_deserializes() {
    // The read-side Flag.status is a raw optional string, so a token the FlagStatus
    // enum does not model must not break bug view.
    let json = r#"{"id": 1, "flags": [{"name": "weird", "status": "??"}]}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.flags[0].status.as_deref(), Some("??"));
}

#[test]
fn history_change_missing_delta_values_stay_unknown() {
    let json = r#"{
        "who": "user@test.com",
        "when": "2025-01-01T00:00:00Z",
        "changes": [{"field_name": "status"}]
    }"#;
    let entry: HistoryEntry = serde_json::from_str(json).unwrap();
    let change = &entry.changes[0];

    assert_eq!(change.removed, None);
    assert_eq!(change.added, None);

    let serialized = serde_json::to_value(entry).unwrap();
    assert!(serialized["changes"][0]["removed"].is_null());
    assert!(serialized["changes"][0]["added"].is_null());
}

#[test]
fn bug_deserializes_custom_fields() {
    let json = r#"{"id": 42, "summary": "s", "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.custom_fields["cf_release"], "9.6");
}

#[test]
fn bug_deserializes_sparse_custom_fields_with_defaults() {
    let json = r#"{"id": 42, "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.id, 42);
    assert!(bug.keywords.is_empty());
    assert_eq!(bug.custom_fields["cf_release"], "9.6");

    let serialized = serde_json::to_value(&bug).unwrap();
    assert_eq!(serialized["summary"], serde_json::Value::Null);
    assert_eq!(serialized["status"], serde_json::Value::Null);
}

#[test]
fn bug_deserialization_drops_non_custom_extension_keys() {
    let json = r#"{"id": 42, "x_extension": "ignored", "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert!(!bug.custom_fields.contains_key("x_extension"));
    assert!(bug.custom_fields.contains_key("cf_release"));
}

#[test]
fn bug_serializes_custom_fields_as_top_level_keys() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    bug.custom_fields
        .insert("cf_release".into(), serde_json::json!("9.6"));

    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["cf_release"], "9.6");
    assert!(serialized.get("custom_fields").is_none());
}

#[test]
fn bug_serialization_drops_non_custom_entries_from_public_map() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    bug.custom_fields
        .insert("cf_release".into(), serde_json::json!("9.6"));
    bug.custom_fields
        .insert("x_extension".into(), serde_json::json!("ignored"));

    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["cf_release"], "9.6");
    assert!(serialized.get("x_extension").is_none());
}

#[test]
fn bug_serializes_custom_fields_after_built_ins_sorted_by_name() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42, "summary": "s"}"#).unwrap();
    bug.custom_fields
        .insert("cf_zeta".into(), serde_json::json!("z"));
    bug.custom_fields
        .insert("cf_alpha".into(), serde_json::json!("a"));

    let serialized = serde_json::to_value(&bug).unwrap();
    let keys: Vec<&str> = serialized
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(&keys[0..3], ["id", "summary", "status"]);
    assert_eq!(&keys[keys.len() - 2..], ["cf_alpha", "cf_zeta"]);
}

#[test]
fn bug_deserializes_dupe_of() {
    let bug: Bug = serde_json::from_value(serde_json::json!({
        "id": 101,
        "summary": "duplicate source",
        "status": "RESOLVED",
        "resolution": "DUPLICATE",
        "dupe_of": 202
    }))
    .unwrap();

    assert_eq!(bug.dupe_of, Some(202));
}

#[test]
fn bug_defaults_missing_dupe_of_to_none() {
    let bug: Bug = serde_json::from_value(serde_json::json!({
        "id": 101,
        "summary": "ordinary bug",
        "status": "NEW"
    }))
    .unwrap();

    assert_eq!(bug.dupe_of, None);
}
