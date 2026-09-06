//! The `BugUpdateDraft` data-transfer struct shared across the `bug update`
//! command modules: built from CLI flags ([`BugUpdateDraft::from_cli`]),
//! deserialized from `--from-json` input, and overlaid with CLI flags
//! ([`BugUpdateDraft::overlay_cli`]) before the payload is built. It is a
//! crate-internal DTO, so its fields are `pub(crate)`.

use crate::cli::UpdateArgs;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use serde::Deserialize;

/// Bug update fields after CLI flags or JSON input have been merged.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BugUpdateDraft {
    pub(crate) id: Option<u64>,
    pub(crate) status: Option<String>,
    pub(crate) resolution: Option<String>,
    pub(crate) dupe_of: Option<u64>,
    pub(crate) alias: Option<String>,
    pub(crate) deadline: Option<String>,
    pub(crate) estimated_time: Option<f64>,
    pub(crate) remaining_time: Option<f64>,
    pub(crate) work_time: Option<f64>,
    pub(crate) reset_assigned_to: Option<bool>,
    pub(crate) reset_qa_contact: Option<bool>,
    pub(crate) assignee: Option<String>,
    pub(crate) platform: Option<String>,
    pub(crate) priority: Option<String>,
    pub(crate) severity: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) whiteboard: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) target_milestone: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) comment_file: Option<std::path::PathBuf>,
    pub(crate) comment_private: Option<bool>,
    #[serde(default)]
    pub(crate) comment_tags: Vec<String>,
    pub(crate) minor_update: Option<bool>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
    #[serde(default)]
    pub(crate) blocks_add: Vec<u64>,
    #[serde(default)]
    pub(crate) blocks_remove: Vec<u64>,
    #[serde(default)]
    pub(crate) depends_on_add: Vec<u64>,
    #[serde(default)]
    pub(crate) depends_on_remove: Vec<u64>,
    #[serde(default)]
    pub(crate) keywords_add: Vec<String>,
    #[serde(default)]
    pub(crate) keywords_remove: Vec<String>,
    #[serde(default)]
    pub(crate) cc_add: Vec<String>,
    #[serde(default)]
    pub(crate) cc_remove: Vec<String>,
    #[serde(default)]
    pub(crate) groups_add: Vec<String>,
    #[serde(default)]
    pub(crate) groups_remove: Vec<String>,
    #[serde(default)]
    pub(crate) see_also_add: Vec<String>,
    #[serde(default)]
    pub(crate) see_also_remove: Vec<String>,
    pub(crate) expect_unchanged_since: Option<String>,
    /// Carried in from the CLI `--field` / `--field-json` overlay, never from
    /// the document — `serde(skip)` keeps it out of the deserialized field
    /// list, so `deny_unknown_fields` still rejects an `extra_fields` key.
    #[serde(skip)]
    pub(crate) extra_fields: crate::types::bug::ExtraBugFields,
}

impl BugUpdateDraft {
    pub(crate) fn from_cli(args: &UpdateArgs) -> Self {
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
            platform: args.platform.clone(),
            priority: args.priority.clone(),
            severity: args.severity.clone(),
            summary: args.summary.clone(),
            whiteboard: args.whiteboard.clone(),
            url: args.url.clone(),
            target_milestone: args.target_milestone.clone(),
            comment: args.comment.clone(),
            comment_file: args.comment_file.clone(),
            comment_private: args.comment_private.then_some(true),
            comment_tags: args.comment_tag.clone(),
            minor_update: args.minor_update.then_some(true),
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
            // Parsing `--field-json` can fail and can read stdin, so the
            // caller does it once and assigns the result.
            extra_fields: crate::types::bug::ExtraBugFields::new(),
        }
    }

    pub(crate) fn overlay_cli(&mut self, args: &UpdateArgs) {
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
        merge_set(&mut self.platform, args.platform.as_deref());
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
        merge_vec(&mut self.comment_tags, &args.comment_tag);
        merge_bool_true(&mut self.minor_update, args.minor_update);
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

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
