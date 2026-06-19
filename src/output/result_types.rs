use std::io::Write;

use serde::Serialize;

use super::formatting::write_json;
use crate::types::OutputFormat;

// ── Result output ───────────────────────────────────────────────────

pub fn write_result<W: Write + ?Sized>(
    value: &(impl Serialize + ?Sized),
    human_message: &str,
    format: OutputFormat,
    out: &mut W,
) {
    match format {
        OutputFormat::Json => write_json(value, out),
        OutputFormat::Table => {
            let _ = writeln!(out, "{human_message}");
        }
    }
}

/// Count-only result for `--count`: serializes as `{"count": N}` under JSON;
/// the table form prints just the integer.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CountResult {
    pub count: usize,
}

impl CountResult {
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self { count }
    }
}

/// Write a count-only result: `{"count": N}` under JSON, a bare integer for
/// the table form. Shared by the `--count` paths of `bug list`/`search`/`my`.
pub fn write_count<W: Write + ?Sized>(count: usize, format: OutputFormat, out: &mut W) {
    write_result(&CountResult::new(count), &count.to_string(), format, out);
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
    #[serde(rename = "renamed")]
    Renamed,
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
    /// Prior alias for a `rename-server` operation; omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_name: Option<String>,
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
            previous_name: None,
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
            previous_name: None,
            url: None,
            is_default: None,
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Updated,
        }
    }

    /// Result for `config remove-server`: the server alias that was removed.
    pub fn removed(name: impl Into<String>, config_file: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            previous_name: None,
            url: None,
            is_default: None,
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Removed,
        }
    }

    /// Result for `config rename-server`: `name` is the new alias and
    /// `previous_name` carries the old one.
    pub fn renamed(
        old_name: impl Into<String>,
        new_name: impl Into<String>,
        config_file: impl Into<String>,
    ) -> Self {
        Self {
            name: new_name.into(),
            previous_name: Some(old_name.into()),
            url: None,
            is_default: None,
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Renamed,
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

/// Typed result payload for batch update operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchResult {
    pub resource: ResourceKind,
    pub action: ActionKind,
    pub succeeded: Vec<u64>,
    pub failed: Vec<BatchFailure>,
}

/// A single failure in a batch operation.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchFailure {
    pub id: u64,
    pub error: String,
}

impl BatchResult {
    pub fn new(succeeded: Vec<u64>, failed: Vec<BatchFailure>) -> Self {
        Self {
            resource: ResourceKind::Bug,
            action: ActionKind::Updated,
            succeeded,
            failed,
        }
    }
}

/// Typed result payload for multi-ID `bzr bug view` JSON output.
///
/// The wrapped shape is used for **every** multi-ID invocation, with or
/// without `--permissive`. `failed` is always present (empty array when
/// no failures) so `jq` consumers can rely on `.bugs[]` and
/// `.failed[]` regardless of arguments. Single-ID `bzr bug view --json`
/// continues to emit a bare `Bug` object — unrelated to this type.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct MultiBugViewResult {
    pub bugs: Vec<crate::types::Bug>,
    pub failed: Vec<BugViewFailure>,
}

/// Per-row failure entry for [`MultiBugViewResult`].
///
/// `id` is `String`, not `u64`, because the caller may have passed an
/// alias (`bzr bug view 12345 my-alias 999`); preserving the original
/// argument lets users correlate failures with the IDs they typed.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BugViewFailure {
    pub id: String,
    pub error: String,
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
#[path = "result_types_tests.rs"]
mod tests;
