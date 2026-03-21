use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateCommentTagsParams {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub is_private: bool,
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
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
}
