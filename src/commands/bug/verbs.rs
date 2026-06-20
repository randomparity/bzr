//! Convenience verbs (`resolve`, `close`, `reopen`, `dup`) — thin sugar over
//! `bug update`. Each builds an `UpdateBugParams` for the common state
//! transition and delegates to the shared `update::apply` path, inheriting its
//! batch, comment, and partial-failure behavior.

use crate::cli::{CloseArgs, CommentArgs, DupArgs, ReopenArgs, ResolveArgs};
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

pub(super) async fn resolve(
    client: &BugzillaClient,
    args: &ResolveArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = UpdateBugParams {
        status: Some("RESOLVED".into()),
        resolution: Some(args.as_resolution.clone()),
        comment: comment_update(&args.comment)?,
        ..Default::default()
    };
    super::update::apply(client, args.ids.clone(), params, format, w).await
}

pub(super) async fn close(
    client: &BugzillaClient,
    args: &CloseArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = UpdateBugParams {
        status: Some("CLOSED".into()),
        resolution: args.as_resolution.clone(),
        comment: comment_update(&args.comment)?,
        ..Default::default()
    };
    super::update::apply(client, args.ids.clone(), params, format, w).await
}

pub(super) async fn reopen(
    client: &BugzillaClient,
    args: &ReopenArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = UpdateBugParams {
        status: Some("REOPENED".into()),
        comment: comment_update(&args.comment)?,
        ..Default::default()
    };
    super::update::apply(client, args.ids.clone(), params, format, w).await
}

pub(super) async fn dup(
    client: &BugzillaClient,
    args: &DupArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = UpdateBugParams {
        dupe_of: Some(args.target),
        comment: comment_update(&args.comment)?,
        ..Default::default()
    };
    super::update::apply(client, vec![args.id], params, format, w).await
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
