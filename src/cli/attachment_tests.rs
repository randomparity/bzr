#![expect(clippy::unwrap_used, clippy::panic)]

use super::{AttachmentAction, UpdateArgs, UploadArgs};
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn attachment_action(args: &[&str]) -> AttachmentAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Attachment { action } => action,
        _ => panic!("expected Commands::Attachment"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_attachment_list_binds_numeric_bug_id() {
    match attachment_action(&["bzr", "attachment", "list", "12345"]) {
        AttachmentAction::List { bug_id, .. } => assert_eq!(bug_id, 12345),
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_attachment_list_rejects_non_numeric_bug_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "attachment", "list", "abc"]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parse_attachment_view_binds_attachment_id() {
    match attachment_action(&["bzr", "attachment", "view", "9876"]) {
        AttachmentAction::View { attachment_id } => assert_eq!(attachment_id, 9876),
        _ => panic!("expected View"),
    }
}

#[test]
fn parse_attachment_download_single_id_defaults_out_dir() {
    match attachment_action(&["bzr", "attachment", "download", "9876"]) {
        AttachmentAction::Download {
            ids,
            bug_ids,
            out,
            out_dir,
        } => {
            assert_eq!(ids, vec![9876]);
            assert!(bug_ids.is_empty());
            assert!(out.is_none());
            assert_eq!(out_dir, "./attachments");
        }
        _ => panic!("expected Download"),
    }
}

#[test]
fn parse_attachment_download_out_to_stdout() {
    match attachment_action(&["bzr", "attachment", "download", "9876", "--out", "-"]) {
        AttachmentAction::Download { ids, out, .. } => {
            assert_eq!(ids, vec![9876]);
            assert_eq!(out.as_deref(), Some("-"));
        }
        _ => panic!("expected Download"),
    }
}

#[test]
fn parse_attachment_download_explicit_out_dir() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "download",
        "9876",
        "9877",
        "--out-dir",
        "/tmp/patches",
    ]) {
        AttachmentAction::Download { ids, out_dir, .. } => {
            assert_eq!(ids, vec![9876, 9877]);
            assert_eq!(out_dir, "/tmp/patches");
        }
        _ => panic!("expected Download"),
    }
}

#[test]
fn parse_attachment_download_bug_flag_is_repeatable() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "download",
        "--bug",
        "12345",
        "--bug",
        "67890",
    ]) {
        AttachmentAction::Download { ids, bug_ids, .. } => {
            assert!(ids.is_empty());
            assert_eq!(bug_ids, vec![12345, 67890]);
        }
        _ => panic!("expected Download"),
    }
}

#[test]
fn parse_attachment_download_mixes_bug_and_positional_ids() {
    match attachment_action(&["bzr", "attachment", "download", "--bug", "12345", "9876"]) {
        AttachmentAction::Download { ids, bug_ids, .. } => {
            assert_eq!(ids, vec![9876]);
            assert_eq!(bug_ids, vec![12345]);
        }
        _ => panic!("expected Download"),
    }
}

#[test]
fn parse_attachment_download_rejects_out_with_out_dir() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "attachment",
            "download",
            "9876",
            "--out",
            "f.diff",
            "--out-dir",
            "/tmp",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_attachment_download_rejects_out_with_bug() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "attachment",
            "download",
            "--bug",
            "12345",
            "--out",
            "f.diff",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_attachment_upload_binds_positionals_with_default_flags() {
    match attachment_action(&["bzr", "attachment", "upload", "12345", "patch.diff"]) {
        AttachmentAction::Upload(UploadArgs {
            bug_id,
            file,
            private,
            no_private,
            patch,
            no_patch,
            comment_private,
            ..
        }) => {
            assert_eq!(bug_id, 12345);
            assert_eq!(file, "patch.diff");
            assert!(!private);
            assert!(!no_private);
            assert!(!patch);
            assert!(!no_patch);
            assert!(!comment_private);
        }
        _ => panic!("expected Upload"),
    }
}

#[test]
fn parse_attachment_upload_no_private_overrides_private() {
    // overrides_with means the later flag wins and resets the other.
    match attachment_action(&[
        "bzr",
        "attachment",
        "upload",
        "12345",
        "patch.diff",
        "--private",
        "--no-private",
    ]) {
        AttachmentAction::Upload(UploadArgs {
            private,
            no_private,
            ..
        }) => {
            assert!(!private);
            assert!(no_private);
        }
        _ => panic!("expected Upload"),
    }
}

#[test]
fn parse_attachment_upload_patch_then_no_patch_last_wins() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "upload",
        "12345",
        "patch.diff",
        "--no-patch",
        "--patch",
    ]) {
        AttachmentAction::Upload(UploadArgs {
            patch, no_patch, ..
        }) => {
            assert!(patch);
            assert!(!no_patch);
        }
        _ => panic!("expected Upload"),
    }
}

#[test]
fn parse_attachment_upload_collects_metadata_and_repeated_flag() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "upload",
        "12345",
        "trace.log",
        "--summary",
        "Stack trace",
        "--content-type",
        "text/plain",
        "--flag",
        "review?",
        "--flag",
        "needinfo?",
    ]) {
        AttachmentAction::Upload(UploadArgs {
            summary,
            content_type,
            flag,
            ..
        }) => {
            assert_eq!(summary.as_deref(), Some("Stack trace"));
            assert_eq!(content_type.as_deref(), Some("text/plain"));
            assert_eq!(flag, vec!["review?", "needinfo?"]);
        }
        _ => panic!("expected Upload"),
    }
}

#[test]
fn parse_attachment_upload_rejects_comment_with_comment_file() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "attachment",
            "upload",
            "12345",
            "fix.patch",
            "--comment",
            "ctx",
            "--comment-file",
            "notes.md",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_attachment_upload_comment_private_with_comment() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "upload",
        "12345",
        "fix.patch",
        "--comment",
        "private context",
        "--comment-private",
    ]) {
        AttachmentAction::Upload(UploadArgs {
            comment,
            comment_private,
            ..
        }) => {
            assert_eq!(comment.as_deref(), Some("private context"));
            assert!(comment_private);
        }
        _ => panic!("expected Upload"),
    }
}

#[test]
fn parse_attachment_update_binds_id_with_unchanged_toggles() {
    match attachment_action(&["bzr", "attachment", "update", "9876", "--summary", "v2"]) {
        AttachmentAction::Update(UpdateArgs {
            id,
            summary,
            obsolete,
            no_obsolete,
            patch,
            no_patch,
            private,
            no_private,
            ..
        }) => {
            assert_eq!(id, 9876);
            assert_eq!(summary.as_deref(), Some("v2"));
            assert!(!obsolete);
            assert!(!no_obsolete);
            assert!(!patch);
            assert!(!no_patch);
            assert!(!private);
            assert!(!no_private);
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_attachment_update_obsolete_and_flag() {
    match attachment_action(&[
        "bzr",
        "attachment",
        "update",
        "9876",
        "--obsolete",
        "--flag",
        "review+",
    ]) {
        AttachmentAction::Update(UpdateArgs {
            obsolete,
            no_obsolete,
            flag,
            ..
        }) => {
            assert!(obsolete);
            assert!(!no_obsolete);
            assert_eq!(flag, vec!["review+"]);
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_attachment_update_requires_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "attachment", "update"]),
        ErrorKind::MissingRequiredArgument
    );
}
