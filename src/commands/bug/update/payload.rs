//! Construction of the `Bug.update` API payload from a merged
//! [`BugUpdateDraft`]: list cleaning, comment resolution, and the assembled
//! [`UpdateBugParams`]. Validation of field combinations runs first via
//! [`super::validate::validate_draft`].

use crate::cli::UpdateArgs;
use crate::error::Result;
use crate::types::bug::{CommentUpdate, IdListUpdate, StringListUpdate, UpdateBugParams};

use super::validate::validate_draft;
use super::BugUpdateDraft;

pub(super) const FLAG_KEYWORDS_ADD: &str = "--keywords-add";
const FLAG_KEYWORDS_REMOVE: &str = "--keywords-remove";
pub(super) const FLAG_CC_ADD: &str = "--cc-add";
const FLAG_CC_REMOVE: &str = "--cc-remove";
pub(super) const FLAG_GROUPS_ADD: &str = "--groups-add";
const FLAG_GROUPS_REMOVE: &str = "--groups-remove";
const FLAG_SEE_ALSO_ADD: &str = "--see-also-add";
pub(super) const FLAG_SEE_ALSO_REMOVE: &str = "--see-also-remove";

fn clean_string_list(field: &str, values: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::error::BzrError::input(format!(
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

pub(crate) fn resolve_comment(
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> Result<Option<CommentUpdate>> {
    let body = crate::commands::runtime::shared::materialize_optional_comment_body(
        comment,
        comment_file,
        comment_private,
    )?;
    let Some(text) = body else {
        return Ok(None);
    };
    Ok(Some(CommentUpdate {
        body: text,
        is_private: comment_private,
    }))
}

pub(super) fn build_update_params(args: &UpdateArgs) -> Result<(Vec<u64>, UpdateBugParams)> {
    build_update_params_from_draft(args.ids.clone(), &BugUpdateDraft::from_cli(args))
}

pub(crate) fn build_update_params_from_draft(
    ids: Vec<u64>,
    draft: &BugUpdateDraft,
) -> Result<(Vec<u64>, UpdateBugParams)> {
    validate_draft(draft, &ids)?;

    let flags = crate::commands::runtime::input::flags::parse_flags(&draft.flags)?;
    let deadline =
        crate::validation::parse_optional_date_only(draft.deadline.as_deref(), "--deadline")?;
    let params = UpdateBugParams {
        status: draft.status.clone(),
        resolution: draft.resolution.clone(),
        dupe_of: draft.dupe_of,
        alias: draft.alias.clone(),
        deadline,
        estimated_time: draft.estimated_time,
        remaining_time: draft.remaining_time,
        work_time: draft.work_time,
        reset_assigned_to: draft.reset_assigned_to.unwrap_or(false),
        reset_qa_contact: draft.reset_qa_contact.unwrap_or(false),
        assigned_to: draft.assignee.clone(),
        platform: draft.platform.clone(),
        priority: draft.priority.clone(),
        severity: draft.severity.clone(),
        summary: draft.summary.clone(),
        whiteboard: draft.whiteboard.clone(),
        url: draft.url.clone(),
        target_milestone: draft.target_milestone.clone(),
        flags,
        blocks: id_list_update(&draft.blocks_add, &draft.blocks_remove),
        depends_on: id_list_update(&draft.depends_on_add, &draft.depends_on_remove),
        keywords: string_list_update(
            FLAG_KEYWORDS_ADD,
            &draft.keywords_add,
            FLAG_KEYWORDS_REMOVE,
            &draft.keywords_remove,
        )?,
        cc: string_list_update(FLAG_CC_ADD, &draft.cc_add, FLAG_CC_REMOVE, &draft.cc_remove)?,
        groups: string_list_update(
            FLAG_GROUPS_ADD,
            &draft.groups_add,
            FLAG_GROUPS_REMOVE,
            &draft.groups_remove,
        )?,
        see_also: string_list_update(
            FLAG_SEE_ALSO_ADD,
            &draft.see_also_add,
            FLAG_SEE_ALSO_REMOVE,
            &draft.see_also_remove,
        )?,
        comment: resolve_comment(
            draft.comment.as_deref(),
            draft.comment_file.as_deref(),
            draft.comment_private.unwrap_or(false),
        )?,
        comment_is_private: std::collections::HashMap::new(),
    };
    if params.is_empty() {
        return Err(crate::error::BzrError::input(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok((ids, params))
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
