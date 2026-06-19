//! Convenience verbs (`resolve`, `close`, `reopen`, `dup`) — thin sugar over
//! `bug update`. Each builds an `UpdateBugParams` for the common state
//! transition and delegates to the shared `update::apply` path, inheriting its
//! batch, comment, and partial-failure behavior.

use crate::cli::{BugAction, CommentArgs};
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::writers::Writers;
use crate::types::{CommentUpdate, OutputFormat, UpdateBugParams};

/// Resolve the `CommentArgs` into an optional `CommentUpdate`, reusing the same
/// stdin/file/private handling as `bug update`.
fn comment_update(args: &CommentArgs) -> Result<Option<CommentUpdate>> {
    super::update::resolve_comment(
        args.comment.as_deref(),
        args.comment_file.as_deref(),
        args.comment_private,
    )
}

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (ids, params) = match action {
        BugAction::Resolve {
            ids,
            as_resolution,
            comment,
        } => (
            ids.clone(),
            UpdateBugParams {
                status: Some("RESOLVED".into()),
                resolution: Some(as_resolution.clone()),
                comment: comment_update(comment)?,
                ..Default::default()
            },
        ),
        BugAction::Close {
            ids,
            as_resolution,
            comment,
        } => (
            ids.clone(),
            UpdateBugParams {
                status: Some("CLOSED".into()),
                resolution: as_resolution.clone(),
                comment: comment_update(comment)?,
                ..Default::default()
            },
        ),
        BugAction::Reopen { ids, comment } => (
            ids.clone(),
            UpdateBugParams {
                status: Some("REOPENED".into()),
                comment: comment_update(comment)?,
                ..Default::default()
            },
        ),
        BugAction::Dup {
            id,
            target,
            comment,
        } => (
            vec![*id],
            UpdateBugParams {
                dupe_of: Some(*target),
                comment: comment_update(comment)?,
                ..Default::default()
            },
        ),
        _ => unreachable!("verbs::handle called with non-verb action"),
    };

    super::update::apply(client, ids, params, format, w).await
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
