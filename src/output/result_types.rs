use serde::Serialize;

use super::formatting::print_json;
use crate::types::OutputFormat;

// ── Result output ───────────────────────────────────────────────────

pub fn print_result(value: &(impl Serialize + ?Sized), human_message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => print_json(value),
        OutputFormat::Table => println!("{human_message}"),
    }
}

// ── Action result types ─────────────────────────────────────────────

/// Resource type for mutation result payloads.
#[derive(Debug, Serialize)]
pub enum ResourceKind {
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "attachment")]
    Attachment,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "product")]
    Product,
    #[serde(rename = "component")]
    Component,
    #[serde(rename = "server")]
    Server,
}

/// Action type for mutation result payloads.
#[derive(Debug, Serialize)]
pub enum ActionKind {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "updated")]
    Updated,
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "removed")]
    Removed,
    #[serde(rename = "downloaded")]
    Downloaded,
}

/// Typed result payload for relationship mutations (e.g. group membership).
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct MembershipResult {
    pub user: String,
    pub group: String,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl MembershipResult {
    pub fn added(user: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            group: group.into(),
            resource: ResourceKind::Group,
            action: ActionKind::Added,
        }
    }

    pub fn removed(user: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            group: group.into(),
            resource: ResourceKind::Group,
            action: ActionKind::Removed,
        }
    }
}

/// Typed result payload for attachment download operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DownloadResult {
    pub id: u64,
    pub file: String,
    pub size: usize,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl DownloadResult {
    pub fn new(id: u64, file: impl Into<String>, size: usize) -> Self {
        Self {
            id,
            file: file.into(),
            size,
            resource: ResourceKind::Attachment,
            action: ActionKind::Downloaded,
        }
    }
}

/// Typed result payload for attachment upload operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct UploadResult {
    pub id: u64,
    pub bug_id: u64,
    pub size: usize,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl UploadResult {
    pub fn new(id: u64, bug_id: u64, size: usize) -> Self {
        Self {
            id,
            bug_id,
            size,
            resource: ResourceKind::Attachment,
            action: ActionKind::Created,
        }
    }
}

/// Typed result payload for comment tag operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TagResult {
    pub comment_id: u64,
    pub tags: Vec<String>,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl TagResult {
    pub fn updated(comment_id: u64, tags: Vec<String>) -> Self {
        Self {
            comment_id,
            tags,
            resource: ResourceKind::Comment,
            action: ActionKind::Updated,
        }
    }
}

/// Typed result payload for config operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ConfigResult {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    pub config_file: String,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl ConfigResult {
    pub fn configured(
        name: impl Into<String>,
        url: impl Into<String>,
        is_default: bool,
        config_file: impl Into<String>,
        is_update: bool,
    ) -> Self {
        Self {
            name: name.into(),
            url: Some(url.into()),
            is_default: Some(is_default),
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: if is_update {
                ActionKind::Updated
            } else {
                ActionKind::Created
            },
        }
    }

    pub fn default_set(name: impl Into<String>, config_file: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            is_default: None,
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Updated,
        }
    }
}

/// Typed result payload for list-shaped search results (e.g. tag search).
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SearchResult {
    pub items: Vec<String>,
}

impl SearchResult {
    pub fn new(items: Vec<String>) -> Self {
        Self { items }
    }
}

/// Typed result payload for JSON output of mutation operations.
///
/// Covers standard CRUD results with an `id` and optional `name`.
/// Relationship mutations use [`MembershipResult`], attachment I/O uses
/// [`DownloadResult`]/[`UploadResult`], tag operations use [`TagResult`],
/// config operations use [`ConfigResult`], and search results use
/// [`SearchResult`].
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ActionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl ActionResult {
    pub fn created(id: u64, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: None,
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn created_named(id: u64, name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: Some(name.into()),
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn updated(id: u64, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: None,
            resource,
            action: ActionKind::Updated,
        }
    }

    pub fn updated_named(name: impl Into<String>, id: Option<u64>, resource: ResourceKind) -> Self {
        Self {
            id,
            name: Some(name.into()),
            resource,
            action: ActionKind::Updated,
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn action_result_created_json_shape() {
        let result = ActionResult::created(42, ResourceKind::Bug);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["id"], 42);
        assert_eq!(json["resource"], "bug");
        assert_eq!(json["action"], "created");
        assert!(json.get("name").is_none());
    }

    #[test]
    fn action_result_created_named_includes_name() {
        let result = ActionResult::created_named(1, "widget", ResourceKind::Component);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["name"], "widget");
        assert_eq!(json["resource"], "component");
    }

    #[test]
    fn upload_result_json_shape() {
        let result = UploadResult::new(10, 42, 1024);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["id"], 10);
        assert_eq!(json["bug_id"], 42);
        assert_eq!(json["size"], 1024);
        assert_eq!(json["resource"], "attachment");
        assert_eq!(json["action"], "created");
    }

    #[test]
    fn download_result_json_shape() {
        let result = DownloadResult::new(5, "patch.diff", 512);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["id"], 5);
        assert_eq!(json["file"], "patch.diff");
        assert_eq!(json["resource"], "attachment");
        assert_eq!(json["action"], "downloaded");
    }

    #[test]
    fn membership_result_added_json_shape() {
        let result = MembershipResult::added("alice", "admin");
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["user"], "alice");
        assert_eq!(json["group"], "admin");
        assert_eq!(json["action"], "added");
    }

    #[test]
    fn tag_result_json_shape() {
        let result = TagResult::updated(7, vec!["important".into()]);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["comment_id"], 7);
        assert_eq!(json["tags"][0], "important");
        assert_eq!(json["action"], "updated");
    }

    #[test]
    fn config_result_skip_none_fields() {
        let result = ConfigResult::default_set("prod", "/etc/bzr/config.toml");
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert!(json.get("url").is_none());
        assert!(json.get("is_default").is_none());
        assert_eq!(json["resource"], "server");
    }

    #[test]
    fn search_result_json_shape() {
        let result = SearchResult::new(vec!["foo".into(), "bar".into()]);
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert_eq!(json["items"][0], "foo");
        assert_eq!(json["items"][1], "bar");
    }

    #[test]
    fn print_result_uses_pretty_json() {
        let result = ActionResult::created(1, ResourceKind::Bug);
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains('\n'), "expected pretty-printed JSON");
    }
}
