use crate::cli::{BugAction, UpdateArgs};
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::bug::UpdateBugParams;
use crate::types::common::OutputFormat;
use serde::Deserialize;

mod output;
mod payload;
mod validate;

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

/// Bug update fields after CLI flags or JSON input have been merged.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BugUpdateDraft {
    pub(super) id: Option<u64>,
    pub(super) status: Option<String>,
    pub(super) resolution: Option<String>,
    pub(super) dupe_of: Option<u64>,
    pub(super) alias: Option<String>,
    pub(super) deadline: Option<String>,
    pub(super) estimated_time: Option<f64>,
    pub(super) remaining_time: Option<f64>,
    pub(super) work_time: Option<f64>,
    pub(super) reset_assigned_to: Option<bool>,
    pub(super) reset_qa_contact: Option<bool>,
    pub(super) assignee: Option<String>,
    pub(super) priority: Option<String>,
    pub(super) severity: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) whiteboard: Option<String>,
    pub(super) url: Option<String>,
    pub(super) target_milestone: Option<String>,
    pub(super) comment: Option<String>,
    pub(super) comment_file: Option<std::path::PathBuf>,
    pub(super) comment_private: Option<bool>,
    #[serde(default)]
    pub(super) flags: Vec<String>,
    #[serde(default)]
    pub(super) blocks_add: Vec<u64>,
    #[serde(default)]
    pub(super) blocks_remove: Vec<u64>,
    #[serde(default)]
    pub(super) depends_on_add: Vec<u64>,
    #[serde(default)]
    pub(super) depends_on_remove: Vec<u64>,
    #[serde(default)]
    pub(super) keywords_add: Vec<String>,
    #[serde(default)]
    pub(super) keywords_remove: Vec<String>,
    #[serde(default)]
    pub(super) cc_add: Vec<String>,
    #[serde(default)]
    pub(super) cc_remove: Vec<String>,
    #[serde(default)]
    pub(super) groups_add: Vec<String>,
    #[serde(default)]
    pub(super) groups_remove: Vec<String>,
    #[serde(default)]
    pub(super) see_also_add: Vec<String>,
    #[serde(default)]
    pub(super) see_also_remove: Vec<String>,
    pub(super) expect_unchanged_since: Option<String>,
}

impl BugUpdateDraft {
    pub(super) fn from_cli(args: &UpdateArgs) -> Self {
        Self {
            id: None,
            status: args.status.clone(),
            resolution: args.resolution.clone(),
            dupe_of: args.dupe_of,
            alias: args.alias.clone(),
            deadline: args.deadline.clone(),
            estimated_time: args.estimated_time,
            remaining_time: args.remaining_time,
            work_time: args.work_time,
            reset_assigned_to: args.reset_assigned_to.then_some(true),
            reset_qa_contact: args.reset_qa_contact.then_some(true),
            assignee: args.assignee.clone(),
            priority: args.priority.clone(),
            severity: args.severity.clone(),
            summary: args.summary.clone(),
            whiteboard: args.whiteboard.clone(),
            url: args.url.clone(),
            target_milestone: args.target_milestone.clone(),
            comment: args.comment.clone(),
            comment_file: args.comment_file.clone(),
            comment_private: args.comment_private.then_some(true),
            flags: args.flag.clone(),
            blocks_add: args.blocks_add.clone(),
            blocks_remove: args.blocks_remove.clone(),
            depends_on_add: args.depends_on_add.clone(),
            depends_on_remove: args.depends_on_remove.clone(),
            keywords_add: args.keywords_add.clone(),
            keywords_remove: args.keywords_remove.clone(),
            cc_add: args.cc_add.clone(),
            cc_remove: args.cc_remove.clone(),
            groups_add: args.groups_add.clone(),
            groups_remove: args.groups_remove.clone(),
            see_also_add: args.see_also_add.clone(),
            see_also_remove: args.see_also_remove.clone(),
            expect_unchanged_since: args.expect_unchanged_since.clone(),
        }
    }

    pub(super) fn overlay_cli(&mut self, args: &UpdateArgs) {
        merge_set(&mut self.status, args.status.as_deref());
        merge_set(&mut self.resolution, args.resolution.as_deref());
        merge_copy(&mut self.dupe_of, args.dupe_of);
        merge_set(&mut self.alias, args.alias.as_deref());
        merge_set(&mut self.deadline, args.deadline.as_deref());
        merge_copy(&mut self.estimated_time, args.estimated_time);
        merge_copy(&mut self.remaining_time, args.remaining_time);
        merge_copy(&mut self.work_time, args.work_time);
        merge_bool_true(&mut self.reset_assigned_to, args.reset_assigned_to);
        merge_bool_true(&mut self.reset_qa_contact, args.reset_qa_contact);
        merge_set(&mut self.assignee, args.assignee.as_deref());
        merge_set(&mut self.priority, args.priority.as_deref());
        merge_set(&mut self.severity, args.severity.as_deref());
        merge_set(&mut self.summary, args.summary.as_deref());
        merge_set(&mut self.whiteboard, args.whiteboard.as_deref());
        merge_set(&mut self.url, args.url.as_deref());
        merge_set(&mut self.target_milestone, args.target_milestone.as_deref());
        if let Some(comment) = args.comment.as_deref() {
            self.comment = Some(comment.to_string());
            self.comment_file = None;
        }
        if let Some(comment_file) = args.comment_file.as_deref() {
            self.comment = None;
            self.comment_file = Some(comment_file.to_path_buf());
        }
        merge_bool_true(&mut self.comment_private, args.comment_private);
        merge_vec(&mut self.flags, &args.flag);
        merge_vec_u64(&mut self.blocks_add, &args.blocks_add);
        merge_vec_u64(&mut self.blocks_remove, &args.blocks_remove);
        merge_vec_u64(&mut self.depends_on_add, &args.depends_on_add);
        merge_vec_u64(&mut self.depends_on_remove, &args.depends_on_remove);
        merge_vec(&mut self.keywords_add, &args.keywords_add);
        merge_vec(&mut self.keywords_remove, &args.keywords_remove);
        merge_vec(&mut self.cc_add, &args.cc_add);
        merge_vec(&mut self.cc_remove, &args.cc_remove);
        merge_vec(&mut self.groups_add, &args.groups_add);
        merge_vec(&mut self.groups_remove, &args.groups_remove);
        merge_vec(&mut self.see_also_add, &args.see_also_add);
        merge_vec(&mut self.see_also_remove, &args.see_also_remove);
        merge_set(
            &mut self.expect_unchanged_since,
            args.expect_unchanged_since.as_deref(),
        );
    }
}

fn merge_copy<T: Copy>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

fn merge_bool_true(target: &mut Option<bool>, value: bool) {
    if value {
        *target = Some(true);
    }
}

fn merge_vec_u64(target: &mut Vec<u64>, value: &[u64]) {
    if !value.is_empty() {
        *target = value.to_vec();
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
