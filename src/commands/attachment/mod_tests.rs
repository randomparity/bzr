use super::{requires_credentials, resolve_bool_flag};
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
