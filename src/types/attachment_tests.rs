#![expect(clippy::unwrap_used)]

use super::*;

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
    assert!(att.is_obsolete);
    assert!(!att.is_private);
}

#[test]
fn bool_from_int_or_bool_deserializes_integers() {
    let json = r#"{"id":1,"bug_id":10,"is_obsolete":1,"is_private":0}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert!(att.is_obsolete);
    assert!(!att.is_private);
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
    assert!(att.is_patch);
}

#[test]
fn is_patch_deserializes_as_int() {
    let json = r#"{"id":1,"bug_id":10,"is_patch":1}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert!(att.is_patch);
}

#[test]
fn is_patch_defaults_to_false_when_absent() {
    let json = r#"{"id":1,"bug_id":10}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert!(!att.is_patch);
}
