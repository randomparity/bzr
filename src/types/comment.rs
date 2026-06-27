use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateCommentTagsParams {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct AddCommentParams {
    #[serde(rename = "comment")]
    pub text: String,
    pub is_private: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Comment {
    pub id: u64,
    /// Parent bug when the server included it; `id` stays required because it
    /// is the primary key.
    #[serde(default)]
    pub bug_id: Option<u64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub is_private: Option<bool>,
    /// Set when the comment was created alongside an attachment via
    /// `Bug.add_attachment`. Used by `attachment upload --comment-private`
    /// to identify the just-created comment.
    #[serde(default)]
    pub attachment_id: Option<u64>,
}

/// Serde JSON keys of [`Comment`], for `--fields` / `--exclude-fields`
/// validation on `comment list`.
pub const COMMENT_FIELDS: &[&str] = &[
    "id",
    "bug_id",
    "text",
    "creator",
    "creation_time",
    "count",
    "is_private",
    "attachment_id",
];

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
