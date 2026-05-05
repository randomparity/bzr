#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn bool_from_int_or_bool_deserializes_true() {
    let json = r#"{"id":1,"is_obsolete":true,"is_private":false}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert!(att.is_obsolete);
    assert!(!att.is_private);
}

#[test]
fn bool_from_int_or_bool_deserializes_integers() {
    let json = r#"{"id":1,"is_obsolete":1,"is_private":0}"#;
    let att: Attachment = serde_json::from_str(json).unwrap();
    assert!(att.is_obsolete);
    assert!(!att.is_private);
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
