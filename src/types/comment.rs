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
    /// Set when the comment was created alongside an attachment via
    /// `Bug.add_attachment`. Used by `attachment upload --comment-private`
    /// to identify the just-created comment.
    #[serde(default)]
    pub attachment_id: Option<u64>,
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
