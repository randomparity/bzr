#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn comment_deserializes_minimal() {
    // Only `id` is required (the primary key). The other fields, including
    // bug_id, fall back to their defaults so off-spec/flat envelopes still
    // parse.
    let json = r#"{"id": 1}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 1);
    assert_eq!(comment.bug_id, 0);
    assert!(comment.text.is_empty());
    assert!(!comment.is_private);
}

#[test]
fn comment_flat_envelope_without_bug_id_still_parses() {
    // The flat `{"comments": [...]}` envelope some Bugzilla 5.0.x servers
    // return (issue #135) may omit bug_id; deserialization must tolerate it
    // rather than fail the whole comment list.
    let json = r#"{"id": 7, "count": 2, "text": "hi", "creator": "a@b.c"}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 7);
    assert_eq!(comment.bug_id, 0);
    assert_eq!(comment.count, 2);
}

#[test]
fn comment_deserializes_full() {
    let json = r#"{"id": 5, "bug_id": 42, "text": "hello", "creator": "alice@test.com", "creation_time": "2024-01-01T00:00:00Z", "count": 3, "is_private": true}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 5);
    assert_eq!(comment.bug_id, 42);
    assert_eq!(comment.text, "hello");
    assert_eq!(comment.creator.as_deref(), Some("alice@test.com"));
    assert_eq!(comment.count, 3);
    assert!(comment.is_private);
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
