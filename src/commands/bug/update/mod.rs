use crate::cli::{BugAction, UpdateArgs};
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::bug::UpdateBugParams;
use crate::types::common::OutputFormat;

mod draft;
mod output;
mod payload;
mod validate;

pub(super) use draft::BugUpdateDraft;
pub(super) use output::write_batch_result;
use output::{comment_suffix, write_update_dry_run};
use payload::build_update_params;
pub(super) use payload::{build_update_params_from_draft, resolve_comment};
use validate::validate_args;

pub(super) fn validate_action(action: &BugAction) -> Result<()> {
    match action {
        BugAction::Update(args) => validate_args(args),
        _ => Ok(()),
    }
}

async fn update_single(
    client: &BugzillaClient,
    id: u64,
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    client.update_bug(id, params).await?;
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

/// The shared exit-11 gate for batch mutations: returns `BatchPartialFailure`
/// (exit 11) when any element failed, else `Ok(())`. Used by batch `bug update`
/// and batch `bug create --from-json`.
pub(super) fn ensure_batch_complete(succeeded: usize, failed: usize) -> Result<()> {
    if failed > 0 {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed })
    } else {
        Ok(())
    }
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
            Ok(()) => succeeded.push(id),
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

pub(super) async fn handle(
    args: &UpdateArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if let Some(arg) = args.from_json.as_deref() {
        return super::update_json::handle(args, arg, ctx, w).await;
    }
    let (ids, params) = build_update_params(args)?;
    apply_checked(
        ApplyRequest {
            ids,
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        ctx,
        w,
    )
    .await
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

pub(super) struct ApplyRequest<'a> {
    pub ids: Vec<u64>,
    pub params: UpdateBugParams,
    pub expect_unchanged_since: Option<&'a str>,
}

/// Apply an update after running the optional optimistic-concurrency guard.
///
/// The guard is skipped under `--dry-run`, which performs no write and whose
/// preview still uses the same payload validation path.
pub(super) async fn apply_checked(
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
pub(super) async fn apply_checked_connected(
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
pub(super) async fn ensure_unchanged_since(
    client: &BugzillaClient,
    ids: &[u64],
    expected: &str,
) -> Result<()> {
    let expected_key = crate::validation::timestamp_compare_key(expected).ok_or_else(|| {
        crate::error::BzrError::InputValidation(format!(
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
/// stdin/TTY into the testable [`crate::commands::runtime::confirm`] primitives. The
/// `should_prompt` gate is checked first, so stdin is locked only when a prompt
/// is actually shown. Returns whether to proceed.
pub(super) fn confirm_batch(count: usize, assume_yes: bool, w: &mut Writers<'_>) -> Result<bool> {
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();
    if !crate::commands::runtime::confirm::should_prompt(count, assume_yes, is_tty) {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    crate::commands::runtime::confirm::read_yes_no(&mut stdin.lock(), w.err, count)
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
