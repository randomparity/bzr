use std::io::Write;

use serde::Serialize;

use super::formatting::write_json_family;
use crate::types::bug::Bug;
use crate::types::output::OutputFormat;

// ── Result output ───────────────────────────────────────────────────

pub fn write_result<W: Write + ?Sized>(
    value: &(impl Serialize + ?Sized),
    human_message: &str,
    format: OutputFormat,
    out: &mut W,
) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => write_json_family(value, format, out),
        OutputFormat::Table => {
            let _ = writeln!(out, "{human_message}");
        }
    }
}

/// Write a "saved <resource>" confirmation. JSON emits
/// `{"name": ..., "action": <verb lowercased>}`; the table form prints
/// `human_message`. Shared by the saved-query and saved-template writers,
/// whose only difference is the human-readable summary line.
pub fn write_saved<W: Write + ?Sized>(
    name: &str,
    verb: &str,
    human_message: &str,
    format: OutputFormat,
    out: &mut W,
) {
    write_result(
        &serde_json::json!({"name": name, "action": verb.to_lowercase()}),
        human_message,
        format,
        out,
    );
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
    #[serde(rename = "dry-run")]
    DryRun,
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
    /// Set when the bug's own field/comment update already succeeded and
    /// only a post-update sub-step (currently just `comment_tags`) failed —
    /// distinct from `id`'s `Bug.update` call itself failing. A caller must
    /// not retry a `comment_tags`-stepped failure with the same `--comment`
    /// text: `Bug.update` posts a new comment on every call, so a retry
    /// would duplicate it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}

impl BatchFailure {
    pub fn new(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            error: error.into(),
            step: None,
        }
    }

    pub fn comment_tags(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            error: error.into(),
            step: Some("comment_tags".to_string()),
        }
    }
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

/// Result of an array (batch) `bug create --from-json`: the IDs of the bugs
/// that were created plus a per-item failure list. Reuses the partial-failure
/// model of batch `bug update`/`bug view`, but the shape differs because a
/// failed *create* has no bug ID — failures carry the **input index** instead,
/// and successes are **new** IDs the server assigned.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchCreateResult {
    pub resource: ResourceKind,
    pub action: ActionKind,
    pub created: Vec<u64>,
    pub failed: Vec<CreateFailure>,
}

/// A single failure in a batch create, identified by its 0-based position in
/// the input array.
///
/// A plain **create** failure (the bug was never filed) serializes as just
/// `{index, error}` — byte-for-byte the original shape. A **sub-step** failure
/// (the bug was filed but its comment or an attachment POST failed) additionally
/// carries `bug_id` (the filed bug, which also appears in
/// [`BatchCreateResult::created`]), `step` (`"comment"`/`"attachment"`), and
/// `file` (the attachment filename, when applicable). The optional fields use
/// `skip_serializing_if` so existing create-failure consumers are unaffected.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateFailure {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub error: String,
}

impl CreateFailure {
    /// A create failure: the bug was never filed.
    pub fn create(index: usize, error: impl Into<String>) -> Self {
        Self {
            index,
            bug_id: None,
            step: None,
            file: None,
            error: error.into(),
        }
    }

    /// A sub-step failure: the bug (`bug_id`) was filed but `step`
    /// (`"comment"`/`"attachment"`) failed; `file` names the attachment when
    /// applicable.
    pub fn sub_step(
        index: usize,
        bug_id: u64,
        step: impl Into<String>,
        file: Option<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            index,
            bug_id: Some(bug_id),
            step: Some(step.into()),
            file,
            error: error.into(),
        }
    }
}

impl BatchCreateResult {
    #[must_use]
    pub fn new(created: Vec<u64>, failed: Vec<CreateFailure>) -> Self {
        Self {
            resource: ResourceKind::Bug,
            action: ActionKind::Created,
            created,
            failed,
        }
    }
}

/// One failed sub-step of a compound `bug create` (the comment or an
/// attachment). `file` is present only for attachment failures.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SubStepFailure {
    pub step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub error: String,
}

impl SubStepFailure {
    pub fn comment(error: impl Into<String>) -> Self {
        Self {
            step: "comment".to_string(),
            file: None,
            error: error.into(),
        }
    }

    pub fn attachment(file: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            step: "attachment".to_string(),
            file: Some(file.into()),
            error: error.into(),
        }
    }

    pub fn comment_tags(error: impl Into<String>) -> Self {
        Self {
            step: "comment_tags".to_string(),
            file: None,
            error: error.into(),
        }
    }
}

/// Result of a single compound `bug create` (flag form or single-object
/// `--from-json`) that filed the bug but had at least one sub-step fail. The
/// created bug `id` is always present (it is the recovery handle); `failed`
/// lists each sub-step failure. Full-success creates emit the plain
/// [`ActionResult`] instead, so this type only appears on partial failure.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CompoundCreateResult {
    pub resource: ResourceKind,
    pub action: ActionKind,
    pub id: u64,
    pub failed: Vec<SubStepFailure>,
}

impl CompoundCreateResult {
    #[must_use]
    pub fn new(id: u64, failed: Vec<SubStepFailure>) -> Self {
        Self {
            resource: ResourceKind::Bug,
            action: ActionKind::Created,
            id,
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
    pub bugs: Vec<Bug>,
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

    pub fn updated_named(id: Option<u64>, name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id,
            name: Some(name.into()),
            resource,
            action: ActionKind::Updated,
        }
    }
}

/// Typed result payload for a `--dry-run` mutation preview.
///
/// Serializes the normal mutation marker (`resource`, `action: "dry-run"`)
/// plus affected existing resource `ids` when available and the `changes`
/// payload that *would* be sent to the write API. `changes` is generic over the
/// request type so create and update payloads can share one result shape
/// without an intermediate `serde_json::Value`.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DryRunResult<'a, P: Serialize> {
    pub resource: ResourceKind,
    pub action: ActionKind,
    pub ids: &'a [u64],
    pub changes: &'a P,
}

impl<'a, P: Serialize> DryRunResult<'a, P> {
    /// Build a dry-run preview for `resource`, listing numeric resource `ids`
    /// when available (empty for name-keyed or create-shaped operations) and
    /// the would-be request `changes`.
    pub fn new(resource: ResourceKind, ids: &'a [u64], changes: &'a P) -> Self {
        Self {
            resource,
            action: ActionKind::DryRun,
            ids,
            changes,
        }
    }
}

#[cfg(test)]
#[path = "result_types_tests.rs"]
mod tests;
