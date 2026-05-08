use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, ResourceKind,
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

fn read_comment_file(path: &std::path::Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        crate::error::BzrError::InputValidation(format!(
            "cannot read --comment-file {}: {e}",
            path.display()
        ))
    })
}

fn resolve_comment(
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> Result<Option<crate::types::CommentUpdate>> {
    let body = match (comment, comment_file) {
        (Some(_), Some(_)) => {
            return Err(crate::error::BzrError::InputValidation(
                "--comment and --comment-file are mutually exclusive".into(),
            ));
        }
        (Some(s), None) => Some(s.to_string()),
        (None, Some(path)) => Some(read_comment_file(path)?),
        (None, None) => None,
    };
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

fn build_update_params(action: &BugAction) -> Result<(Vec<u64>, UpdateBugParams)> {
    let BugAction::Update {
        ids,
        status,
        resolution,
        dupe_of,
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
    } = action
    else {
        unreachable!()
    };

    let flags = crate::commands::flags::parse_flags(flag)?;
    let params = UpdateBugParams {
        status: status.clone(),
        resolution: resolution.clone(),
        dupe_of: *dupe_of,
        assigned_to: assignee.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        summary: summary.clone(),
        whiteboard: whiteboard.clone(),
        flags,
        blocks: IdListUpdate {
            add: blocks_add.clone(),
            remove: blocks_remove.clone(),
        },
        depends_on: IdListUpdate {
            add: depends_on_add.clone(),
            remove: depends_on_remove.clone(),
        },
        keywords: StringListUpdate {
            add: clean_string_list(FLAG_KEYWORDS_ADD, keywords_add)?,
            remove: clean_string_list(FLAG_KEYWORDS_REMOVE, keywords_remove)?,
        },
        cc: StringListUpdate {
            add: clean_string_list(FLAG_CC_ADD, cc_add)?,
            remove: clean_string_list(FLAG_CC_REMOVE, cc_remove)?,
        },
        groups: StringListUpdate {
            add: clean_string_list(FLAG_GROUPS_ADD, groups_add)?,
            remove: clean_string_list(FLAG_GROUPS_REMOVE, groups_remove)?,
        },
        see_also: StringListUpdate {
            add: clean_string_list(FLAG_SEE_ALSO_ADD, see_also_add)?,
            remove: clean_string_list(FLAG_SEE_ALSO_REMOVE, see_also_remove)?,
        },
        comment: resolve_comment(
            comment.as_deref(),
            comment_file.as_deref(),
            *comment_private,
        )?,
        comment_is_private: std::collections::HashMap::new(),
    };
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
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format, w).await
    } else {
        update_batch(client, &ids, &params, format, w).await
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
