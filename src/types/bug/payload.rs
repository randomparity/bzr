use serde::Serialize;

use crate::types::common::FlagUpdate;

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct CreateBugParams {
    pub product: String,
    pub component: String,
    pub summary: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_sys: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rep_platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
}

/// Represents an incremental update to a list field (blocks, `depends_on`).
/// Bugzilla accepts `{ "add": [...], "remove": [...] }` for these fields.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct IdListUpdate {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<u64>,
}

impl IdListUpdate {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// Represents an incremental update to a string-typed list field
/// (keywords, cc, groups, `see_also`). Bugzilla accepts
/// `{ "add": [...], "remove": [...] }` for these fields.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct StringListUpdate {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

impl StringListUpdate {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// A comment to post atomically with a `Bug.update` call.
///
/// Serializes as `{"body": "...", "is_private": <bool>}`. Bugzilla's
/// REST `Bug.update` accepts this as a sub-object on the request,
/// which delivers the field changes and the comment in one
/// round-trip.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct CommentUpdate {
    pub body: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_private: bool,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateBugParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dupe_of: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_time: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reset_assigned_to: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reset_qa_contact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
    #[serde(skip_serializing_if = "IdListUpdate::is_empty")]
    pub blocks: IdListUpdate,
    #[serde(skip_serializing_if = "IdListUpdate::is_empty")]
    pub depends_on: IdListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub keywords: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub cc: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub groups: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub see_also: StringListUpdate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<CommentUpdate>,
    /// Edit the privacy of comments that are already on the bug.
    /// Keys are comment IDs; values are `true` (mark private) or
    /// `false` (mark public). Used by `attachment upload
    /// --comment-private` to flip the privacy of the comment created
    /// by `Bug.add_attachment`.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub comment_is_private: std::collections::HashMap<u64, bool>,
}

impl UpdateBugParams {
    /// True when no field would be sent to the server - i.e. the params
    /// serialize to an empty JSON object. Every field declares
    /// `skip_serializing_if`, so serialization is the single source of
    /// truth for "no changes requested" and stays correct as fields are
    /// added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().map(serde_json::Map::is_empty))
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
