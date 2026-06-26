#![expect(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Direct tests for [`super::build_update_params`] and
//! [`super::build_update_params_from_draft`]: scalar/list/comment payload
//! construction, list cleaning, and no-op rejection.

use crate::cli::BugAction;

use super::super::test_helpers::{
    make_empty_update_action, make_update_action, make_update_action_with_comment,
    make_update_action_with_dupe_of, make_update_action_with_scalar_parity_fields, update_ids_mut,
};
use super::{
    build_update_params, build_update_params_from_draft, BugUpdateDraft, FLAG_CC_ADD,
    FLAG_GROUPS_ADD, FLAG_KEYWORDS_ADD, FLAG_SEE_ALSO_REMOVE,
};

/// Borrow the inner `UpdateArgs` from a `BugAction::Update` test fixture.
fn as_update_args(action: &BugAction) -> &crate::cli::UpdateArgs {
    match action {
        BugAction::Update(args) => args,
        _ => panic!("expected BugAction::Update"),
    }
}

fn update_deadline_mut(action: &mut BugAction) -> Option<&mut Option<String>> {
    let BugAction::Update(crate::cli::UpdateArgs { deadline, .. }) = action else {
        return None;
    };
    Some(deadline)
}

#[derive(Default)]
struct UpdateLists<'a> {
    keywords_add: Vec<&'a str>,
    keywords_remove: Vec<&'a str>,
    cc_add: Vec<&'a str>,
    cc_remove: Vec<&'a str>,
    groups_add: Vec<&'a str>,
    groups_remove: Vec<&'a str>,
    see_also_add: Vec<&'a str>,
    see_also_remove: Vec<&'a str>,
}

fn make_update_action_with_lists(lists: UpdateLists<'_>) -> BugAction {
    let to_strings = |v: Vec<&str>| v.into_iter().map(String::from).collect();
    BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![1],
        keywords_add: to_strings(lists.keywords_add),
        keywords_remove: to_strings(lists.keywords_remove),
        cc_add: to_strings(lists.cc_add),
        cc_remove: to_strings(lists.cc_remove),
        groups_add: to_strings(lists.groups_add),
        groups_remove: to_strings(lists.groups_remove),
        see_also_add: to_strings(lists.see_also_add),
        see_also_remove: to_strings(lists.see_also_remove),
        ..Default::default()
    })
}

#[test]
fn build_update_params_populates_string_lists() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["fix-needed"],
        cc_add: vec!["alice@example.com"],
        groups_remove: vec!["secret"],
        see_also_add: vec!["https://example.com/issue/1"],
        ..UpdateLists::default()
    });
    let (ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(ids, vec![1]);
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.cc.add, vec!["alice@example.com"]);
    assert_eq!(params.groups.remove, vec!["secret"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_populates_dupe_of() {
    let action = make_update_action_with_dupe_of(42, 99);
    let (ids, params) = build_update_params(as_update_args(&action)).unwrap();

    assert_eq!(ids, vec![42]);
    assert_eq!(params.dupe_of, Some(99));
    assert!(params.status.is_none());
    assert!(params.resolution.is_none());
}

#[test]
fn build_update_params_populates_scalar_parity_fields() {
    let action = make_update_action_with_scalar_parity_fields();
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();

    assert_eq!(params.alias.as_deref(), Some("short-name"));
    assert_eq!(params.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(params.estimated_time, Some(3.5));
    assert_eq!(params.remaining_time, Some(1.25));
    assert_eq!(params.work_time, Some(0.5));
    assert_eq!(params.url.as_deref(), Some("https://example.com/repro"));
    assert_eq!(params.target_milestone.as_deref(), Some("5.0"));
    assert!(params.reset_assigned_to);
    assert!(params.reset_qa_contact);
}

#[test]
fn build_update_params_accepts_valid_deadline_verbatim() {
    let mut action = make_update_action(vec![42]);
    *update_deadline_mut(&mut action).expect("update action") = Some("2026-12-31".into());

    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    // Date-only deadlines must reach the server unchanged (no datetime expansion).
    assert_eq!(params.deadline.as_deref(), Some("2026-12-31"));
}

#[test]
fn build_update_params_rejects_invalid_deadline() {
    let mut action = make_update_action(vec![42]);
    *update_deadline_mut(&mut action).expect("update action") = Some("garbage".into());

    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--deadline")),
        "expected --deadline validation error, got {err:?}"
    );
}

#[test]
fn build_update_params_rejects_alias_with_multiple_ids() {
    let mut action = make_update_action_with_scalar_parity_fields();
    *update_ids_mut(&mut action).expect("expected update action") = vec![42, 43];

    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--alias")),
        "expected --alias validation error, got {err:?}"
    );
}

#[test]
fn build_update_params_from_draft_validates_ids_passed_separately() {
    let draft = BugUpdateDraft {
        alias: Some("short-name".into()),
        ..Default::default()
    };

    let err = build_update_params_from_draft(vec![42, 43], &draft).unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--alias")),
        "expected --alias validation error, got {err:?}"
    );
}

#[test]
fn build_update_params_trims_string_list_values() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["  fix-needed  "],
        see_also_add: vec!["  https://example.com/issue/1  "],
        ..UpdateLists::default()
    });
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_populates_keywords_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_remove: vec!["stale"],
        ..UpdateLists::default()
    });
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.keywords.remove, vec!["stale"]);
    assert!(params.keywords.add.is_empty());
}

#[test]
fn build_update_params_populates_cc_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        cc_remove: vec!["bob@example.com"],
        ..UpdateLists::default()
    });
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.cc.remove, vec!["bob@example.com"]);
    assert!(params.cc.add.is_empty());
}

#[test]
fn build_update_params_populates_groups_add() {
    let action = make_update_action_with_lists(UpdateLists {
        groups_add: vec!["secret"],
        ..UpdateLists::default()
    });
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.groups.add, vec!["secret"]);
    assert!(params.groups.remove.is_empty());
}

#[test]
fn build_update_params_populates_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["https://example.com/issue/2"],
        ..UpdateLists::default()
    });
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.see_also.remove, vec!["https://example.com/issue/2"]);
    assert!(params.see_also.add.is_empty());
}

#[test]
fn build_update_params_rejects_empty_keyword() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["", "fix-needed"],
        ..UpdateLists::default()
    });
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(FLAG_KEYWORDS_ADD)),
        "expected InputValidation naming {FLAG_KEYWORDS_ADD}, got {err:?}",
    );
}

#[test]
fn build_update_params_rejects_empty_keyword_remove_with_remove_flag() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_remove: vec![" "],
        ..UpdateLists::default()
    });
    let err = build_update_params(as_update_args(&action)).unwrap_err();

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("--keywords-remove")),
        "expected InputValidation naming --keywords-remove, got {err:?}",
    );
}

#[test]
fn build_update_params_rejects_whitespace_only_cc() {
    let action = make_update_action_with_lists(UpdateLists {
        cc_add: vec!["   "],
        ..UpdateLists::default()
    });
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(FLAG_CC_ADD)),
        "expected InputValidation naming {FLAG_CC_ADD}, got {err:?}",
    );
}

#[test]
fn build_update_params_rejects_empty_groups_add() {
    let action = make_update_action_with_lists(UpdateLists {
        groups_add: vec![""],
        ..UpdateLists::default()
    });
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(FLAG_GROUPS_ADD)),
        "expected InputValidation naming {FLAG_GROUPS_ADD}, got {err:?}",
    );
}

#[test]
fn build_update_params_rejects_whitespace_only_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["   "],
        ..UpdateLists::default()
    });
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(FLAG_SEE_ALSO_REMOVE)),
        "expected InputValidation naming {FLAG_SEE_ALSO_REMOVE}, got {err:?}",
    );
}

#[test]
fn build_update_params_carries_public_comment() {
    let action = make_update_action_with_comment(vec![1], Some("hello"), None, false);
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "hello");
    assert!(!comment.is_private);
}

#[test]
fn build_update_params_carries_private_comment() {
    let action = make_update_action_with_comment(vec![1], Some("secret"), None, true);
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "secret");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_omits_comment_when_unspecified() {
    let mut action = make_update_action_with_comment(vec![1], None, None, false);
    if let BugAction::Update(crate::cli::UpdateArgs { status, .. }) = &mut action {
        *status = Some("CONFIRMED".into());
    }
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    assert!(params.comment.is_none());
}

#[test]
fn build_update_params_rejects_private_without_body() {
    let action = make_update_action_with_comment(vec![1], None, None, true);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("--comment-private"),
        "error should mention the flag: {msg}"
    );
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
}

#[test]
fn build_update_params_rejects_whitespace_only_comment() {
    let action = make_update_action_with_comment(vec![1], Some("   \n\t"), None, false);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::InputValidation(ref m) if m.contains("empty comment")
    ));
}

#[test]
fn build_update_params_rejects_comment_and_comment_file_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "from a file").unwrap();
    let action = make_update_action_with_comment(vec![1], Some("inline"), Some(&path), false);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    match err {
        crate::error::BzrError::InputValidation(msg) => {
            assert!(msg.contains("--comment"), "names inline flag: {msg}");
            assert!(msg.contains("--comment-file"), "names file flag: {msg}");
        }
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn build_update_params_reads_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "from a file").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "from a file");
    assert!(!comment.is_private);
}

#[test]
fn build_update_params_comment_file_with_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "private body").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), true);
    let (_ids, params) = build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_rejects_missing_comment_file() {
    let path = std::path::Path::new("/nonexistent/bzr-issue-161-test.txt");
    let action = make_update_action_with_comment(vec![1], None, Some(path), false);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
    assert!(
        msg.contains("/nonexistent/bzr-issue-161-test.txt"),
        "error should include path: {msg}"
    );
}

#[test]
fn build_update_params_rejects_non_utf8_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.bin");
    std::fs::write(&path, [0xff_u8, 0xfe, 0xfd]).unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
}

#[test]
fn build_update_params_rejects_whitespace_only_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "   \n\t  \n").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::InputValidation(ref m) if m.contains("empty comment")
    ));
}

#[test]
fn build_update_params_rejects_update_with_no_fields() {
    let action = make_empty_update_action(vec![42]);
    let err = build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("at least one")),
        "expected an at-least-one-field validation error, got {err:?}"
    );
}
