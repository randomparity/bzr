//! Direct tests for [`super::BugUpdateDraft::from_cli`] and
//! [`super::BugUpdateDraft::overlay_cli`]: CLI-to-draft field mapping, the
//! CLI-over-JSON precedence rule, and the comment vs comment-file override.

use std::path::PathBuf;

use crate::cli::UpdateArgs;

use super::BugUpdateDraft;

#[test]
fn from_cli_maps_scalar_fields() {
    let args = UpdateArgs {
        ids: vec![7],
        status: Some("RESOLVED".into()),
        resolution: Some("FIXED".into()),
        dupe_of: Some(99),
        deadline: Some("2026-12-31".into()),
        estimated_time: Some(3.5),
        assignee: Some("alice@example.com".into()),
        priority: Some("high".into()),
        summary: Some("new summary".into()),
        url: Some("https://example.com/repro".into()),
        target_milestone: Some("5.0".into()),
        comment_tag: vec!["triaged".into(), "needs-review".into()],
        minor_update: true,
        ..Default::default()
    };
    let draft = BugUpdateDraft::from_cli(&args);

    // The positional ids are tracked separately, not on the draft.
    assert_eq!(draft.id, None);
    assert_eq!(draft.status.as_deref(), Some("RESOLVED"));
    assert_eq!(draft.resolution.as_deref(), Some("FIXED"));
    assert_eq!(draft.dupe_of, Some(99));
    assert_eq!(draft.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(draft.estimated_time, Some(3.5));
    assert_eq!(draft.assignee.as_deref(), Some("alice@example.com"));
    assert_eq!(draft.priority.as_deref(), Some("high"));
    assert_eq!(draft.summary.as_deref(), Some("new summary"));
    assert_eq!(draft.url.as_deref(), Some("https://example.com/repro"));
    assert_eq!(draft.target_milestone.as_deref(), Some("5.0"));
    assert_eq!(draft.comment_tags, vec!["triaged", "needs-review"]);
    assert_eq!(draft.minor_update, Some(true));
}

#[test]
fn from_cli_omits_minor_update_when_false() {
    let args = UpdateArgs {
        ids: vec![7],
        minor_update: false,
        ..Default::default()
    };
    let draft = BugUpdateDraft::from_cli(&args);
    assert_eq!(draft.minor_update, None);
}

#[test]
fn overlay_cli_replaces_comment_tags_and_sets_minor_update() {
    let mut draft = BugUpdateDraft {
        comment_tags: vec!["json-tag".into()],
        ..Default::default()
    };
    let args = UpdateArgs {
        ids: vec![7],
        comment_tag: vec!["cli-tag".into()],
        minor_update: true,
        ..Default::default()
    };
    draft.overlay_cli(&args);
    assert_eq!(draft.comment_tags, vec!["cli-tag"]);
    assert_eq!(draft.minor_update, Some(true));
}

#[test]
fn overlay_cli_preserves_json_comment_tags_when_cli_empty() {
    let mut draft = BugUpdateDraft {
        comment_tags: vec!["json-tag".into()],
        minor_update: Some(true),
        ..Default::default()
    };
    let args = UpdateArgs {
        ids: vec![7],
        ..Default::default()
    };
    draft.overlay_cli(&args);
    assert_eq!(draft.comment_tags, vec!["json-tag"]);
    assert_eq!(draft.minor_update, Some(true));
}

#[test]
fn from_cli_maps_list_fields() {
    let args = UpdateArgs {
        ids: vec![1],
        keywords_add: vec!["fix-needed".into()],
        cc_remove: vec!["bob@example.com".into()],
        blocks_add: vec![100, 200],
        depends_on_remove: vec![50],
        see_also_add: vec!["https://example.com/issue/1".into()],
        flag: vec!["review?".into()],
        ..Default::default()
    };
    let draft = BugUpdateDraft::from_cli(&args);

    assert_eq!(draft.keywords_add, vec!["fix-needed"]);
    assert_eq!(draft.cc_remove, vec!["bob@example.com"]);
    assert_eq!(draft.blocks_add, vec![100, 200]);
    assert_eq!(draft.depends_on_remove, vec![50]);
    assert_eq!(draft.see_also_add, vec!["https://example.com/issue/1"]);
    assert_eq!(draft.flags, vec!["review?"]);
}

#[test]
fn from_cli_maps_comment_fields() {
    let args = UpdateArgs {
        ids: vec![1],
        comment: Some("looks good".into()),
        comment_private: true,
        ..Default::default()
    };
    let draft = BugUpdateDraft::from_cli(&args);

    assert_eq!(draft.comment.as_deref(), Some("looks good"));
    assert_eq!(draft.comment_file, None);
    assert_eq!(draft.comment_private, Some(true));
}

#[test]
fn from_cli_bool_flags_become_option_true_only_when_set() {
    let args = UpdateArgs {
        ids: vec![1],
        reset_assigned_to: true,
        reset_qa_contact: false,
        comment_private: false,
        ..Default::default()
    };
    let draft = BugUpdateDraft::from_cli(&args);

    assert_eq!(draft.reset_assigned_to, Some(true));
    assert_eq!(draft.reset_qa_contact, None);
    assert_eq!(draft.comment_private, None);
}

#[test]
fn overlay_cli_overrides_json_scalar() {
    let mut draft = BugUpdateDraft {
        status: Some("NEW".into()),
        ..Default::default()
    };
    let args = UpdateArgs {
        status: Some("ASSIGNED".into()),
        ..Default::default()
    };
    draft.overlay_cli(&args);

    assert_eq!(draft.status.as_deref(), Some("ASSIGNED"));
}

#[test]
fn overlay_cli_preserves_json_when_cli_absent() {
    let mut draft = BugUpdateDraft {
        status: Some("NEW".into()),
        ..Default::default()
    };
    draft.overlay_cli(&UpdateArgs::default());

    assert_eq!(draft.status.as_deref(), Some("NEW"));
}

#[test]
fn overlay_cli_comment_flag_clears_comment_file() {
    let mut draft = BugUpdateDraft {
        comment_file: Some(PathBuf::from("/tmp/json-body.txt")),
        ..Default::default()
    };
    let args = UpdateArgs {
        comment: Some("inline body".into()),
        ..Default::default()
    };
    draft.overlay_cli(&args);

    assert_eq!(draft.comment.as_deref(), Some("inline body"));
    assert_eq!(draft.comment_file, None);
}

#[test]
fn overlay_cli_comment_file_flag_clears_comment() {
    let mut draft = BugUpdateDraft {
        comment: Some("json body".into()),
        ..Default::default()
    };
    let args = UpdateArgs {
        comment_file: Some(PathBuf::from("/tmp/cli-body.txt")),
        ..Default::default()
    };
    draft.overlay_cli(&args);

    assert_eq!(draft.comment, None);
    assert_eq!(draft.comment_file, Some(PathBuf::from("/tmp/cli-body.txt")));
}

#[test]
fn overlay_cli_list_replaces_when_cli_present() {
    let mut draft = BugUpdateDraft {
        keywords_add: vec!["json-keyword".into()],
        ..Default::default()
    };
    let args = UpdateArgs {
        keywords_add: vec!["cli-keyword".into()],
        ..Default::default()
    };
    draft.overlay_cli(&args);

    assert_eq!(draft.keywords_add, vec!["cli-keyword"]);
}

#[test]
fn overlay_cli_list_preserved_when_cli_empty() {
    let mut draft = BugUpdateDraft {
        keywords_add: vec!["json-keyword".into()],
        ..Default::default()
    };
    draft.overlay_cli(&UpdateArgs::default());

    assert_eq!(draft.keywords_add, vec!["json-keyword"]);
}
