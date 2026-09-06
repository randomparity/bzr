//! Execution of `bug update`: single and batch writes, the
//! `--expect-unchanged-since` optimistic-concurrency guard, batch confirmation,
//! and the `apply_checked` orchestration entry points shared with the
//! convenience verbs and `--from-json`.

use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::mutation::ensure_batch_complete;
use crate::error::{BzrError, Result};
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::bug::UpdateBugParams;
use crate::types::comment::UpdateCommentTagsParams;
use crate::types::output::OutputFormat;

use super::output::{comment_suffix, write_batch_result, write_update_dry_run};

/// Bugzilla's `Bug.update` `comment_tags` parameter is not reliably honored
/// across supported server versions (issue #672: confirmed silently ignored
/// on a live Bugzilla 5.0.6), unlike `Bug.create`'s hard rejection of the
/// same parameter. Tag the just-posted comment directly via the
/// confirmed-working `bug/comment/{id}/tags` endpoint instead of relying on
/// it. `resolve_comment_tags` (in `payload.rs`) guarantees `params.comment`
/// is `Some` whenever `comment_tags` is non-empty.
async fn apply_comment_tags(
    client: &BugzillaClient,
    bug_id: u64,
    params: &UpdateBugParams,
) -> Result<()> {
    let Some(comment) = params.comment.as_ref() else {
        return Ok(());
    };
    let comments = client.get_comments_since(bug_id, None).await?;
    let comment_id = comments
        .iter()
        .rev()
        .find(|c| c.text.as_deref() == Some(comment.body.as_str()))
        .map(|c| c.id)
        .ok_or_else(|| BzrError::NotFound {
            resource: "comment",
            id: bug_id.to_string(),
        })?;
    let tag_params = UpdateCommentTagsParams {
        add: params.comment_tags.clone(),
        remove: vec![],
    };
    client.update_comment_tags(comment_id, &tag_params).await?;
    Ok(())
}

async fn update_single(
    client: &BugzillaClient,
    id: u64,
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    client.update_bug(id, params).await?;
    if !params.comment_tags.is_empty() {
        apply_comment_tags(client, id, params).await?;
    }
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_result(
                &ActionResult::updated(id, ResourceKind::Bug),
                "",
                format,
                w.out,
            );
        }
        OutputFormat::Table => {
            let suffix = comment_suffix(params.comment.is_some());
            let _ = writeln!(w.out, "Updated bug #{id}{suffix}");
        }
    }
    Ok(())
}

async fn update_batch(
    client: &BugzillaClient,
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for &id in ids {
        match client.update_bug(id, params).await {
            Ok(()) if params.comment_tags.is_empty() => succeeded.push(id),
            Ok(()) => match apply_comment_tags(client, id, params).await {
                Ok(()) => succeeded.push(id),
                Err(e) => failed.push(BatchFailure {
                    id,
                    error: e.to_string(),
                }),
            },
            Err(e) => failed.push(BatchFailure {
                id,
                error: e.to_string(),
            }),
        }
    }
    let batch = BatchResult::new(succeeded, failed);
    write_batch_result(&batch, format, params.comment.is_some(), w);
    ensure_batch_complete(batch.succeeded.len(), batch.failed.len())
}

async fn apply_connected(
    client: &BugzillaClient,
    ids: Vec<u64>,
    params: UpdateBugParams,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    if !confirm_batch(ids.len(), ctx.assume_yes(), w)? {
        let _ = writeln!(w.err, "Aborted; no changes made.");
        return Ok(());
    }
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format, w).await
    } else {
        update_batch(client, &ids, &params, format, w).await
    }
}

pub(crate) struct ApplyRequest<'a> {
    pub ids: Vec<u64>,
    pub params: UpdateBugParams,
    pub expect_unchanged_since: Option<&'a str>,
}

/// Apply an update after running the optional optimistic-concurrency guard.
///
/// The guard is skipped under `--dry-run`, which performs no write and whose
/// preview still uses the same payload validation path.
pub(crate) async fn apply_checked(
    request: ApplyRequest<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if ctx.dry_run() {
        write_update_dry_run(&request.ids, &request.params, ctx.format(), w);
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    apply_checked_connected(&client, request, ctx, w).await
}

/// Apply an update with an already configured client after running the optional
/// optimistic-concurrency guard.
pub(crate) async fn apply_checked_connected(
    client: &BugzillaClient,
    request: ApplyRequest<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    if ctx.dry_run() {
        write_update_dry_run(&request.ids, &request.params, format, w);
        return Ok(());
    }
    if let Some(expected) = request.expect_unchanged_since {
        ensure_unchanged_since(client, &request.ids, expected).await?;
    }
    apply_connected(client, request.ids, request.params, ctx, w).await
}

/// Optimistic-concurrency guard for `--expect-unchanged-since`: refuse the
/// update if any target bug's current `last_change_time` differs from
/// `expected`. The check is client-side — Bugzilla's REST `Bug.update` has no
/// atomic compare-and-set — so a narrow window remains between this re-read and
/// the write. All IDs are checked before any write, so a batch is
/// all-or-nothing on collision.
pub(crate) async fn ensure_unchanged_since(
    client: &BugzillaClient,
    ids: &[u64],
    expected: &str,
) -> Result<()> {
    let expected_key = crate::validation::timestamp_compare_key(expected).ok_or_else(|| {
        crate::error::BzrError::input(format!(
            "--expect-unchanged-since: '{expected}' is not a recognized UTC timestamp; \
             pass the last_change_time value from a preceding `bug view`"
        ))
    })?;
    for &id in ids {
        let bug = client
            .get_bug(&id.to_string(), Some("id,last_change_time"), None)
            .await?;
        let actual = bug.last_change_time.ok_or_else(|| {
            crate::error::BzrError::DataIntegrity(format!(
                "bug {id} returned no last_change_time; cannot verify --expect-unchanged-since"
            ))
        })?;
        let actual_key = crate::validation::timestamp_compare_key(&actual).ok_or_else(|| {
            crate::error::BzrError::DataIntegrity(format!(
                "bug {id} returned an unrecognized last_change_time '{actual}'"
            ))
        })?;
        if actual_key != expected_key {
            return Err(crate::error::BzrError::MidAirCollision {
                id,
                expected: expected.to_string(),
                actual,
            });
        }
    }
    Ok(())
}

/// Prompt for confirmation before a large batch mutation, wiring the real
/// stdin/TTY into the testable [`crate::commands::runtime::interaction::confirm`] primitives. The
/// `should_prompt` gate is checked first, so stdin is locked only when a prompt
/// is actually shown. Returns whether to proceed.
pub(crate) fn confirm_batch(count: usize, assume_yes: bool, w: &mut Writers<'_>) -> Result<bool> {
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();
    if !crate::commands::runtime::interaction::confirm::should_prompt(count, assume_yes, is_tty) {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    crate::commands::runtime::interaction::confirm::read_yes_no(&mut stdin.lock(), w.err, count)
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;
