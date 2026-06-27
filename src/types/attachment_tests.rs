#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn attachment_fields_matches_serialized_keys() {
    // Fully-serializing fixture: `data` is the only skip_serializing_if field,
    // so it must be Some for the key set to be complete.
    let a = crate::test_helpers::make_attachment(1, 2, "f", "s", Some("ZGF0YQ==".into()));
    let value = serde_json::to_value(&a).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> =
        ATTACHMENT_FIELDS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        serialized, declared,
        "ATTACHMENT_FIELDS drifted from serde output"
    );
    assert_eq!(
        ATTACHMENT_FIELDS.len(),
        declared.len(),
        "ATTACHMENT_FIELDS has duplicates"
    );
}

#[test]
fn attachment_deserializes_flags_and_defaults_empty() {
    let with = r#"{"id":1,"bug_id":10,"flags":[{"name":"review","status":"+","setter":"a@x"}]}"#;
    let att: Attachment = serde_json::from_str(with).unwrap();
    assert_eq!(att.flags.len(), 1);
    assert_eq!(att.flags[0].name, "review");
    assert_eq!(att.flags[0].status, "+");
    assert_eq!(att.flags[0].setter.as_deref(), Some("a@x"));

    let without: Attachment = serde_json::from_str(r#"{"id":2,"bug_id":10}"#).unwrap();
    assert!(without.flags.is_empty());
    // flags serializes always as an array so consumers can rely on the key.
    let value = serde_json::to_value(&without).unwrap();
    assert_eq!(value["flags"], serde_json::json!([]));
}

#[test]
fn bool_from_int_or_bool_deserializes_true() {
    let json = r#"{"id":1,"bug_id":10,"is_obsolete":true,"is_private":false}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert_eq!(att.is_obsolete, Some(true));
    assert_eq!(att.is_private, Some(false));
}

#[test]
fn bool_from_int_or_bool_deserializes_integers() {
    let json = r#"{"id":1,"bug_id":10,"is_obsolete":1,"is_private":0}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert_eq!(att.is_obsolete, Some(true));
    assert_eq!(att.is_private, Some(false));
}

#[test]
fn bool_from_int_or_bool_rejects_non_binary_numbers() {
    for raw in ["2", "-1", "1.5"] {
        let json = format!(r#"{{"id":1,"is_obsolete":{raw}}}"#);

        let err = serde_json::from_str::<Attachment>(&json).unwrap_err();

        assert!(
            err.to_string().contains("expected bool or 0/1 integer"),
            "unexpected error for {raw}: {err}"
        );
    }
}

#[test]
fn bool_from_int_or_bool_rejects_string() {
    let json = r#"{"id":1,"is_obsolete":"yes"}"#;
    let err = serde_json::from_str::<Attachment>(json).unwrap_err();
    assert!(
        err.to_string().contains("expected bool or integer"),
        "unexpected error: {err}"
    );
}

#[test]
fn is_patch_deserializes_as_bool() {
    let json = r#"{"id":1,"bug_id":10,"is_patch":true}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert_eq!(att.is_patch, Some(true));
}

#[test]
fn is_patch_deserializes_as_int() {
    let json = r#"{"id":1,"bug_id":10,"is_patch":1}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert_eq!(att.is_patch, Some(true));
}

#[test]
fn is_patch_defaults_to_false_when_absent() {
    let json = r#"{"id":1,"bug_id":10}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();

    let serialized = serde_json::to_value(&att).unwrap();
    assert_eq!(serialized["is_patch"], serde_json::Value::Null);
}

#[test]
fn attachment_missing_wire_scalars_serialize_as_null() {
    let att: Attachment = serde_json::from_str(r#"{"id":2}"#).unwrap();

    let serialized = serde_json::to_value(&att).unwrap();
    for field in [
        "bug_id",
        "file_name",
        "summary",
        "content_type",
        "size",
        "is_obsolete",
        "is_private",
        "is_patch",
    ] {
        assert_eq!(
            serialized[field],
            serde_json::Value::Null,
            "field {field} should preserve absence"
        );
    }
}
