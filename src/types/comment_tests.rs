#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn comment_deserializes_minimal() {
    let json = r#"{"id": 1}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 1);

    let serialized = serde_json::to_value(&comment).unwrap();
    assert_eq!(serialized["bug_id"], serde_json::Value::Null);
    assert_eq!(serialized["text"], serde_json::Value::Null);
    assert_eq!(serialized["count"], serde_json::Value::Null);
    assert_eq!(serialized["is_private"], serde_json::Value::Null);
}

#[test]
fn comment_flat_envelope_without_bug_id_still_parses() {
    // The flat `{"comments": [...]}` envelope some Bugzilla 5.0.x servers
    // return (issue #135) may omit bug_id; deserialization must tolerate it
    // rather than fail the whole comment list.
    let json = r#"{"id": 7, "count": 2, "text": "hi", "creator": "a@b.c"}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 7);

    let serialized = serde_json::to_value(&comment).unwrap();
    assert_eq!(serialized["bug_id"], serde_json::Value::Null);
    assert_eq!(serialized["count"], 2);
}

#[test]
fn comment_deserializes_full() {
    let json = r#"{"id": 5, "bug_id": 42, "text": "hello", "creator": "alice@test.com", "creation_time": "2024-01-01T00:00:00Z", "count": 3, "is_private": true}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 5);
    assert_eq!(comment.bug_id, Some(42));
    assert_eq!(comment.text.as_deref(), Some("hello"));
    assert_eq!(comment.creator.as_deref(), Some("alice@test.com"));
    assert_eq!(comment.count, Some(3));
    assert_eq!(comment.is_private, Some(true));
}

#[test]
fn comment_is_private_accepts_integer() {
    for (json, expected) in [
        (r#"{"id": 5, "is_private": 0}"#, Some(false)),
        (r#"{"id": 5, "is_private": 1}"#, Some(true)),
    ] {
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.is_private, expected, "{json}");
    }
}

#[test]
fn comment_is_private_accepts_bool_null_and_absence() {
    for (json, expected) in [
        (r#"{"id": 5, "is_private": false}"#, Some(false)),
        (r#"{"id": 5, "is_private": true}"#, Some(true)),
        (r#"{"id": 5, "is_private": null}"#, None),
        (r#"{"id": 5}"#, None),
    ] {
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.is_private, expected, "{json}");
    }
}

#[test]
fn comment_is_private_rejects_unchartered_values() {
    for json in [
        r#"{"id": 5, "is_private": 2}"#,
        r#"{"id": 5, "is_private": -1}"#,
        r#"{"id": 5, "is_private": "1"}"#,
    ] {
        assert!(
            serde_json::from_str::<Comment>(json).is_err(),
            "is_private must reject {json}"
        );
    }
}

#[test]
fn comment_deserializes_with_attachment_id() {
    let json = r#"{"id": 7, "bug_id": 10, "attachment_id": 99}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 7);
    assert_eq!(comment.attachment_id, Some(99));
}

#[test]
fn comment_deserializes_without_attachment_id_defaults_to_none() {
    let json = r#"{"id": 8, "bug_id": 10}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 8);
    assert!(comment.attachment_id.is_none());
}

#[test]
fn comment_fields_matches_serialized_keys() {
    let c = Comment {
        id: 1,
        bug_id: Some(2),
        text: Some("t".into()),
        creator: Some("c".into()),
        creation_time: Some("2020".into()),
        count: Some(0),
        is_private: Some(false),
        attachment_id: Some(3),
    };
    let value = serde_json::to_value(&c).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> =
        COMMENT_FIELDS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        serialized, declared,
        "COMMENT_FIELDS drifted from serde output"
    );
    assert_eq!(
        COMMENT_FIELDS.len(),
        declared.len(),
        "COMMENT_FIELDS has duplicates"
    );
}
