use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{IdListUpdate, OutputFormat, StringListUpdate, UpdateBugParams};

const FLAG_KEYWORDS_ADD: &str = "--keywords-add";
const FLAG_KEYWORDS_REMOVE: &str = "--keywords-remove";
const FLAG_CC_ADD: &str = "--cc-add";
const FLAG_CC_REMOVE: &str = "--cc-remove";
const FLAG_GROUPS_ADD: &str = "--groups-add";
const FLAG_GROUPS_REMOVE: &str = "--groups-remove";
const FLAG_SEE_ALSO_ADD: &str = "--see-also-add";
const FLAG_SEE_ALSO_REMOVE: &str = "--see-also-remove";

const COMMENT_SUFFIX: &str = " (with comment)";

fn comment_suffix(present: bool) -> &'static str {
    if present {
        COMMENT_SUFFIX
    } else {
        ""
    }
}

fn clean_string_list(field: &str, values: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::error::BzrError::InputValidation(format!(
                "{field}: list value cannot be empty or whitespace-only"
            )));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}

/// Build an `IdListUpdate` from the raw `--*-add` / `--*-remove` ID lists.
fn id_list_update(add: &[u64], remove: &[u64]) -> IdListUpdate {
    IdListUpdate {
        add: add.to_vec(),
        remove: remove.to_vec(),
    }
}

/// Build a `StringListUpdate`, validating each side via [`clean_string_list`].
/// `add_flag` / `remove_flag` name the originating CLI flags for error context.
fn string_list_update(
    add_flag: &str,
    add: &[String],
    remove_flag: &str,
    remove: &[String],
) -> Result<StringListUpdate> {
    Ok(StringListUpdate {
        add: clean_string_list(add_flag, add)?,
        remove: clean_string_list(remove_flag, remove)?,
    })
}

pub(super) fn resolve_comment(
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> Result<Option<crate::types::CommentUpdate>> {
    let body = crate::commands::shared::materialize_body_source(
        crate::commands::shared::classify_body_source(
            comment,
            comment_file,
            "--comment",
            "--comment-file",
        )?,
        "--comment-file",
    )?;
    if body.is_none() && comment_private {
        return Err(crate::error::BzrError::InputValidation(
            "--comment-private requires --comment or --comment-file".into(),
        ));
    }
    let Some(text) = body else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "empty comment, aborting".into(),
        ));
    }
    Ok(Some(crate::types::CommentUpdate {
        body: text,
        is_private: comment_private,
    }))
}

pub(super) fn validate_action(action: &BugAction) -> Result<()> {
    match action {
        BugAction::Update {
            ids,
            alias: Some(_),
            ..
        } if ids.len() > 1 => Err(crate::error::BzrError::InputValidation(
            "--alias can only be used when updating one bug".into(),
        )),
        _ => Ok(()),
    }
}

fn build_update_params(action: &BugAction) -> Result<(Vec<u64>, UpdateBugParams)> {
    validate_action(action)?;

    let BugAction::Update {
        ids,
        status,
        resolution,
        dupe_of,
        alias,
        deadline,
        estimated_time,
        remaining_time,
        work_time,
        reset_assigned_to,
        reset_qa_contact,
        assignee,
        priority,
        severity,
        summary,
        whiteboard,
        flag,
        blocks_add,
        blocks_remove,
        depends_on_add,
        depends_on_remove,
        keywords_add,
        keywords_remove,
        cc_add,
        cc_remove,
        groups_add,
        groups_remove,
        see_also_add,
        see_also_remove,
        comment,
        comment_file,
        comment_private,
        expect_unchanged_since: _,
    } = action
    else {
        unreachable!()
    };

    let flags = crate::commands::flags::parse_flags(flag)?;
    let deadline = crate::validation::parse_optional_date_only(deadline.as_deref(), "--deadline")?;
    let params = UpdateBugParams {
        status: status.clone(),
        resolution: resolution.clone(),
        dupe_of: *dupe_of,
        alias: alias.clone(),
        deadline,
        estimated_time: *estimated_time,
        remaining_time: *remaining_time,
        work_time: *work_time,
        reset_assigned_to: *reset_assigned_to,
        reset_qa_contact: *reset_qa_contact,
        assigned_to: assignee.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        summary: summary.clone(),
        whiteboard: whiteboard.clone(),
        flags,
        blocks: id_list_update(blocks_add, blocks_remove),
        depends_on: id_list_update(depends_on_add, depends_on_remove),
        keywords: string_list_update(
            FLAG_KEYWORDS_ADD,
            keywords_add,
            FLAG_KEYWORDS_REMOVE,
            keywords_remove,
        )?,
        cc: string_list_update(FLAG_CC_ADD, cc_add, FLAG_CC_REMOVE, cc_remove)?,
        groups: string_list_update(
            FLAG_GROUPS_ADD,
            groups_add,
            FLAG_GROUPS_REMOVE,
            groups_remove,
        )?,
        see_also: string_list_update(
            FLAG_SEE_ALSO_ADD,
            see_also_add,
            FLAG_SEE_ALSO_REMOVE,
            see_also_remove,
        )?,
        comment: resolve_comment(
            comment.as_deref(),
            comment_file.as_deref(),
            *comment_private,
        )?,
        comment_is_private: std::collections::HashMap::new(),
    };
    if params.is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok((ids.clone(), params))
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
        OutputFormat::Json => {
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

fn write_batch_result(
    batch: &BatchResult,
    format: OutputFormat,
    with_comment: bool,
    w: &mut Writers<'_>,
) {
    match format {
        OutputFormat::Json => {
            write_result(batch, "", format, w.out);
        }
        OutputFormat::Table => {
            if !batch.succeeded.is_empty() {
                let ids_str: Vec<String> =
                    batch.succeeded.iter().map(|id| format!("#{id}")).collect();
                let suffix = comment_suffix(with_comment);
                let _ = writeln!(w.out, "Updated bugs: {}{suffix}", ids_str.join(", "));
            }
            for f in &batch.failed {
                let _ = writeln!(w.err, "Failed to update bug #{}: {}", f.id, f.error);
            }
        }
    }
}

fn ensure_batch_complete(succeeded: usize, failed: usize) -> Result<()> {
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
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (ids, params) = build_update_params(action)?;
    let BugAction::Update {
        expect_unchanged_since,
        ..
    } = action
    else {
        unreachable!()
    };
    // Optimistic-concurrency guard, before any write. Skipped under --dry-run,
    // which performs no write (and `apply` short-circuits to a preview anyway).
    if let Some(expected) = expect_unchanged_since {
        if !crate::commands::dry_run::enabled() {
            ensure_unchanged_since(client, &ids, expected).await?;
        }
    }
    apply(client, ids, params, format, w).await
}

/// Emit the would-be update without writing: the affected IDs and the payload
/// that would be sent, marked `"action":"dry-run"`. Shared by `bug update` and
/// the convenience verbs.
fn write_update_dry_run(
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    let ids_str: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    let suffix = comment_suffix(params.comment.is_some());
    write_result(
        &DryRunResult::new(ResourceKind::Bug, ids, params),
        &format!(
            "Dry run: would update bug(s) {}{suffix} (no changes made)",
            ids_str.join(", ")
        ),
        format,
        w.out,
    );
}

/// Apply an already-built `UpdateBugParams` to one or more bug IDs, dispatching
/// to the single- or batch-update path. Shared by `bug update` and the
/// convenience verbs (`resolve`/`close`/`reopen`/`dup`). Under `--dry-run`,
/// previews the change without calling the write API. The optimistic-concurrency
/// guard (`--expect-unchanged-since`) runs in `handle`, before this is called.
pub(super) async fn apply(
    client: &BugzillaClient,
    ids: Vec<u64>,
    params: UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::dry_run::enabled() {
        write_update_dry_run(&ids, &params, format, w);
        return Ok(());
    }
    if !confirm_batch(ids.len(), w)? {
        let _ = writeln!(w.err, "Aborted; no changes made.");
        return Ok(());
    }
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format, w).await
    } else {
        update_batch(client, &ids, &params, format, w).await
    }
}

/// Optimistic-concurrency guard for `--expect-unchanged-since`: refuse the
/// update if any target bug's current `last_change_time` differs from
/// `expected`. The check is client-side — Bugzilla's REST `Bug.update` has no
/// atomic compare-and-set — so a narrow window remains between this re-read and
/// the write. All IDs are checked before any write, so a batch is
/// all-or-nothing on collision.
async fn ensure_unchanged_since(
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
/// stdin/TTY into the testable [`crate::commands::confirm`] primitives. The
/// `should_prompt` gate is checked first, so stdin is locked only when a prompt
/// is actually shown. Returns whether to proceed.
fn confirm_batch(count: usize, w: &mut Writers<'_>) -> Result<bool> {
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();
    if !crate::commands::confirm::should_prompt(count, crate::commands::confirm::yes(), is_tty) {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    crate::commands::confirm::read_yes_no(&mut stdin.lock(), w.err, count)
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
