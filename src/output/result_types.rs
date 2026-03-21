use serde::Serialize;

use crate::types::OutputFormat;

// ── Result output ───────────────────────────────────────────────────

pub fn print_result(value: &(impl Serialize + ?Sized), human_message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(value).expect("serializable to JSON")
            );
        }
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

    pub fn updated_named(name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id: None,
            name: Some(name.into()),
            resource,
            action: ActionKind::Updated,
        }
    }
}
