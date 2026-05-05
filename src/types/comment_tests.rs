#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn comment_deserializes_minimal() {
    let json = r#"{"id": 1}"#;
    let comment: Comment = serde_json::from_str(json).unwrap();
    assert_eq!(comment.id, 1);
    assert_eq!(comment.bug_id, 0);
    assert!(comment.text.is_empty());
    assert!(!comment.is_private);
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
