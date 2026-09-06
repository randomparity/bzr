use serde::{Serialize, Serializer};

use crate::types::flag::FlagUpdate;

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CreateBugParams {
    pub product: String,
    pub component: String,
    pub summary: String,
    pub version: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub severity: Option<String>,
    pub assigned_to: Option<String>,
    pub op_sys: Option<String>,
    pub platform: Option<String>,
    pub alias: Option<String>,
    pub url: Option<String>,
    pub whiteboard: Option<String>,
    pub target_milestone: Option<String>,
    pub deadline: Option<String>,
    pub blocks: Vec<u64>,
    pub depends_on: Vec<u64>,
    pub cc: Vec<String>,
    pub keywords: Vec<String>,
    pub groups: Vec<String>,
    pub(crate) groups_present: bool,
    pub flags: Vec<FlagUpdate>,
}

impl CreateBugParams {
    pub(crate) fn set_groups_from_structured_input(&mut self, groups: Vec<String>) {
        self.groups = groups;
        self.groups_present = true;
    }
}

#[derive(Serialize)]
struct CreateBugParamsWire<'a> {
    product: &'a String,
    component: &'a String,
    summary: &'a String,
    version: &'a String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    severity: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assigned_to: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_sys: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    whiteboard: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_milestone: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    blocks: &'a Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    depends_on: &'a Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cc: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keywords: &'a Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    groups: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    flags: &'a Vec<FlagUpdate>,
}

impl Serialize for CreateBugParams {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        CreateBugParamsWire {
            product: &self.product,
            component: &self.component,
            summary: &self.summary,
            version: &self.version,
            description: self.description.as_deref(),
            priority: self.priority.as_deref(),
            severity: self.severity.as_deref(),
            assigned_to: self.assigned_to.as_deref(),
            op_sys: self.op_sys.as_deref(),
            platform: self.platform.as_deref(),
            alias: self.alias.as_deref(),
            url: self.url.as_deref(),
            whiteboard: self.whiteboard.as_deref(),
            target_milestone: self.target_milestone.as_deref(),
            deadline: self.deadline.as_deref(),
            blocks: &self.blocks,
            depends_on: &self.depends_on,
            cc: &self.cc,
            keywords: &self.keywords,
            groups: (self.groups_present || !self.groups.is_empty())
                .then_some(self.groups.as_slice()),
            flags: &self.flags,
        }
        .serialize(serializer)
    }
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
    pub platform: Option<String>,
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
    /// Tags applied to the comment created by `comment` above. Bugzilla only
    /// applies these when `comment` is also present.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comment_tags: Vec<String>,
    /// Suppress bugmail notifications for this update.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub minor_update: bool,
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
