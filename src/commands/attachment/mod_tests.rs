#![expect(clippy::unwrap_used)]

use super::{requires_credentials, resolve_bool_flag, update_has_changes, validate_action};
use crate::cli::{AttachmentAction, AttachmentUpdateArgs, UploadArgs};

fn upload_args() -> UploadArgs {
    UploadArgs {
        bug_id: 1,
        file: "f.txt".into(),
        summary: None,
        content_type: None,
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: false,
        flag: vec![],
    }
}

fn update_args() -> AttachmentUpdateArgs {
    AttachmentUpdateArgs {
        id: 1,
        summary: None,
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    }
}

#[test]
fn resolve_bool_flag_yes_wins() {
    assert_eq!(resolve_bool_flag(true, false), Some(true));
}

#[test]
fn resolve_bool_flag_no_resolves_false() {
    assert_eq!(resolve_bool_flag(false, true), Some(false));
}

#[test]
fn resolve_bool_flag_neither_is_none() {
    assert_eq!(resolve_bool_flag(false, false), None);
}

#[test]
fn requires_credentials_reads_need_none() {
    assert!(requires_credentials(&AttachmentAction::List { bug_id: 1 }).is_none());
    assert!(requires_credentials(&AttachmentAction::View { attachment_id: 1 }).is_none());
    assert!(requires_credentials(&AttachmentAction::Download {
        ids: vec![1],
        bug_ids: vec![],
        out: None,
        out_dir: "x".into(),
    })
    .is_none());
}

#[test]
fn requires_credentials_writes_name_the_command() {
    assert_eq!(
        requires_credentials(&AttachmentAction::Upload(upload_args())),
        Some("attachment upload")
    );
    assert_eq!(
        requires_credentials(&AttachmentAction::Update(update_args())),
        Some("attachment update")
    );
}

// ── update_has_changes (|| vs && mutants) ────────────────────────────────────
//
// Each test sets EXACTLY one field and asserts update_has_changes returns true.
// Replacing any single `||` with `&&` would require all other fields to also be
// set, making these single-field tests fail under the && mutation.

#[test]
fn update_has_changes_summary_alone_is_true() {
    let args = AttachmentUpdateArgs {
        summary: Some("new summary".into()),
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_file_name_alone_is_true() {
    let args = AttachmentUpdateArgs {
        file_name: Some("new_name.txt".into()),
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_content_type_alone_is_true() {
    let args = AttachmentUpdateArgs {
        content_type: Some("application/octet-stream".into()),
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_obsolete_alone_is_true() {
    let args = AttachmentUpdateArgs {
        obsolete: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_no_obsolete_alone_is_true() {
    let args = AttachmentUpdateArgs {
        no_obsolete: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_patch_alone_is_true() {
    let args = AttachmentUpdateArgs {
        patch: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_no_patch_alone_is_true() {
    let args = AttachmentUpdateArgs {
        no_patch: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_private_alone_is_true() {
    let args = AttachmentUpdateArgs {
        private: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_no_private_alone_is_true() {
    let args = AttachmentUpdateArgs {
        no_private: true,
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_flag_alone_is_true() {
    let args = AttachmentUpdateArgs {
        flag: vec!["review+".into()],
        ..update_args()
    };
    assert!(update_has_changes(&args));
}

#[test]
fn update_has_changes_all_none_is_false() {
    // Sanity: a fully empty args struct has no changes.
    assert!(!update_has_changes(&update_args()));
}

// ── validate_action upload --comment-private match arm ───────────────────────
//
// Mutant: delete the match arm that rejects Upload with comment_private=true
// but no comment/comment_file. Without that arm, the validation silently
// passes — the test below must fail under the mutant.

#[test]
fn validate_action_rejects_upload_comment_private_without_body() {
    let args = UploadArgs {
        comment_private: true,
        comment: None,
        comment_file: None,
        ..upload_args()
    };
    let action = AttachmentAction::Upload(args);
    let result = validate_action(&action);
    assert!(
        result.is_err(),
        "--comment-private with no comment body must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("--comment-private"),
        "error should name the flag, got: {msg}"
    );
}

#[test]
fn validate_action_allows_upload_comment_private_with_comment() {
    let args = UploadArgs {
        comment_private: true,
        comment: Some("a body".into()),
        comment_file: None,
        ..upload_args()
    };
    let action = AttachmentAction::Upload(args);
    assert!(
        validate_action(&action).is_ok(),
        "--comment-private + --comment should pass validation"
    );
}

#[test]
fn validate_action_allows_upload_comment_private_with_comment_file() {
    let args = UploadArgs {
        comment_private: true,
        comment: None,
        comment_file: Some("/tmp/body.txt".into()),
        ..upload_args()
    };
    let action = AttachmentAction::Upload(args);
    assert!(
        validate_action(&action).is_ok(),
        "--comment-private + --comment-file should pass validation"
    );
}
