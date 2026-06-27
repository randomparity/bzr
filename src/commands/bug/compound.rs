//! Compound `bug create`: file a bug, then post its first comment and any
//! attachments in the same logical operation.
//!
//! The plan ([`CompoundPlan`]) is built and fully validated **before** the bug
//! is created (files read, comment body materialized), so a missing-file or
//! empty-comment typo fails as input validation without filing an unfinishable
//! bug. After a successful create, [`run_sub_steps`] posts the comment and each
//! attachment in order, collecting failures rather than rolling back (see
//! ADR-0012): the created bug ID is the recovery handle. Any post-create
//! sub-step failure surfaces the ID (stdout result + stderr warning) and exits
//! `11` (`BatchPartialFailure`).

use serde::Serialize;

use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::mutation::ensure_batch_complete;
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, CompoundCreateResult, DryRunResult, ResourceKind, SubStepFailure,
};
use crate::output::writers::Writers;
use crate::types::attachment::UploadAttachmentParams;
use crate::types::bug::CreateBugParams;
use crate::types::comment::AddCommentParams;

/// The comment and attachments to attach to a freshly-created bug. Built before
/// any network call; an empty plan means there are no sub-steps to run.
#[derive(Debug)]
pub(super) struct CompoundPlan {
    pub comment: Option<AddCommentParams>,
    pub attachments: Vec<UploadAttachmentParams>,
}

impl CompoundPlan {
    /// True when there is no comment and no attachment — the caller can use the
    /// plain single-create path with byte-identical output.
    pub fn is_empty(&self) -> bool {
        self.comment.is_none() && self.attachments.is_empty()
    }
}

/// Post the comment (if any) then each attachment, in order, against the
/// already-created `bug_id`. Never aborts early: every failure is recorded and
/// announced on stderr (naming the bug ID) so one run reports the complete
/// failure set. Consumes the plan so attachment payloads are not cloned.
pub(super) async fn run_sub_steps(
    client: &BugzillaClient,
    bug_id: u64,
    plan: CompoundPlan,
    w: &mut Writers<'_>,
) -> Vec<SubStepFailure> {
    let mut failures = Vec::new();
    if let Some(comment) = plan.comment {
        if let Err(e) = client.add_comment(bug_id, &comment).await {
            let _ = writeln!(
                w.err,
                "warning: created bug #{bug_id} but failed to add comment: {e}"
            );
            failures.push(SubStepFailure::comment(e.to_string()));
        }
    }
    for mut attachment in plan.attachments {
        let file = attachment.file_name.clone();
        attachment.bug_id = bug_id;
        if let Err(e) = client.upload_attachment(&attachment).await {
            let _ = writeln!(
                w.err,
                "warning: created bug #{bug_id} but failed to upload attachment '{file}': {e}"
            );
            failures.push(SubStepFailure::attachment(file, e.to_string()));
        }
    }
    failures
}

/// Create one bug and run its compound sub-steps (the single-bug scope: flag
/// form and single-object `--from-json`).
///
/// On a dry run, prints the full planned payload (bug + comment + attachments)
/// and returns without connecting. Otherwise creates the bug, runs the
/// sub-steps, and either writes the plain success [`ActionResult`] (no sub-step
/// failed) or the [`CompoundCreateResult`] and returns `BatchPartialFailure`
/// (exit 11).
pub(super) async fn create_with_sub_steps(
    params: &CreateBugParams,
    plan: CompoundPlan,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    if ctx.dry_run() {
        write_compound_dry_run(params, &plan, ctx, w);
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_bug(params).await?;
    let failures = run_sub_steps(&client, id, plan, w).await;
    if failures.is_empty() {
        write_result(
            &ActionResult::created(id, ResourceKind::Bug),
            &format!("Created bug #{id}"),
            format,
            w.out,
        );
        return Ok(());
    }
    let failed_count = failures.len();
    write_result(
        &CompoundCreateResult::new(id, failures),
        &format!("Created bug #{id}"),
        format,
        w.out,
    );
    ensure_batch_complete(1, failed_count)
}

/// Serializable `changes` payload for a compound dry run: the bug fields plus
/// the resolved comment body and attachment metadata.
#[derive(Serialize)]
pub(super) struct CompoundDryRunChanges<'a> {
    #[serde(flatten)]
    bug: &'a CreateBugParams,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<AttachmentDryRun<'a>>,
}

/// One attachment's dry-run preview (no payload bytes, just metadata + size).
#[derive(Serialize)]
struct AttachmentDryRun<'a> {
    file_name: &'a str,
    summary: &'a str,
    content_type: &'a str,
    size: usize,
}

/// Build the dry-run `changes` view for one bug + its compound plan. Shared by
/// the single-scope dry run and the array-scope dry run (which collects a `Vec`
/// of these).
pub(super) fn dry_run_changes<'a>(
    params: &'a CreateBugParams,
    plan: &'a CompoundPlan,
) -> CompoundDryRunChanges<'a> {
    CompoundDryRunChanges {
        bug: params,
        comment: plan.comment.as_ref().map(|c| c.text.as_str()),
        attachments: plan
            .attachments
            .iter()
            .map(|a| AttachmentDryRun {
                file_name: &a.file_name,
                summary: &a.summary,
                content_type: &a.content_type,
                size: a.data.len(),
            })
            .collect(),
    }
}

/// Emit the would-be compound payload without writing, marked
/// `"action":"dry-run"`.
fn write_compound_dry_run(
    params: &CreateBugParams,
    plan: &CompoundPlan,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) {
    let changes = dry_run_changes(params, plan);
    write_result(
        &DryRunResult::new(ResourceKind::Bug, &[], &changes),
        &format!(
            "Dry run: would create a bug in {}/{} with {} comment and {} attachment(s) (nothing created)",
            params.product,
            params.component,
            if plan.comment.is_some() { "1" } else { "0" },
            plan.attachments.len(),
        ),
        ctx.format(),
        w.out,
    );
}

#[cfg(test)]
#[path = "compound_tests.rs"]
mod tests;
