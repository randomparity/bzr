//! Convenience verbs (`resolve`, `close`, `reopen`, `dup`) — thin sugar over
//! `bug update`. Each builds an `UpdateBugParams` for the common state
//! transition and delegates to the shared `update::apply` path, inheriting its
//! batch, comment, and partial-failure behavior.

use crate::cli::{CloseArgs, CommentArgs, DupArgs, ReopenArgs, ResolveArgs};
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
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

/// Confirm the `close` / `reopen` target status exists on the server before
/// writing, so an unknown status fails with an actionable client-side error
/// (exit 7) rather than the server's opaque "no status named X" API error.
///
/// The match is exact and case-sensitive against the names the server returns
/// (Bugzilla statuses are uppercase). The check proves the status *exists*; the
/// legality of the transition from the bug's current status is still left to
/// the server. Skipped under `--dry-run`, which performs no mutation and whose
/// preview already shows the status that would be sent.
async fn validate_target_status(client: &BugzillaClient, status: &str) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        return Ok(());
    }
    if status.trim().is_empty() {
        return Err(BzrError::InputValidation("--status cannot be empty".into()));
    }
    let values = client.get_field_values("status").await?;
    if values
        .iter()
        .any(|v| !v.name.is_empty() && v.name == status)
    {
        return Ok(());
    }
    let valid: Vec<&str> = values
        .iter()
        .map(|v| v.name.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    Err(BzrError::InputValidation(format!(
        "no status named '{status}' on this server; valid statuses: {}",
        valid.join(", ")
    )))
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
    super::update::apply_checked(
        client,
        super::update::ApplyRequest {
            ids: args.ids.clone(),
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        format,
        w,
    )
    .await
}

pub(super) async fn close(
    client: &BugzillaClient,
    args: &CloseArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    // Resolve the comment (local validation) before the network status check so
    // a bad --comment-private combination fails without a round-trip.
    let comment = comment_update(&args.comment)?;
    validate_target_status(client, &args.status).await?;
    let params = UpdateBugParams {
        status: Some(args.status.clone()),
        resolution: args.as_resolution.clone(),
        comment,
        ..Default::default()
    };
    super::update::apply_checked(
        client,
        super::update::ApplyRequest {
            ids: args.ids.clone(),
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        format,
        w,
    )
    .await
}

pub(super) async fn reopen(
    client: &BugzillaClient,
    args: &ReopenArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let comment = comment_update(&args.comment)?;
    validate_target_status(client, &args.status).await?;
    let params = UpdateBugParams {
        status: Some(args.status.clone()),
        comment,
        ..Default::default()
    };
    super::update::apply_checked(
        client,
        super::update::ApplyRequest {
            ids: args.ids.clone(),
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        format,
        w,
    )
    .await
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
    super::update::apply_checked(
        client,
        super::update::ApplyRequest {
            ids: vec![args.id],
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        format,
        w,
    )
    .await
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
