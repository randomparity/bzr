#![expect(clippy::unwrap_used)]

use std::io::Write;
use std::path::Path;

use super::{guess_content_type, prepare_attachment_params, AttachmentInput};

fn tmp_file(name: &str, contents: &[u8]) -> tempfile::TempPath {
    let mut f = tempfile::Builder::new()
        .prefix("bzr-att-")
        .suffix(name)
        .tempfile()
        .unwrap();
    f.write_all(contents).unwrap();
    f.into_temp_path()
}

#[test]
fn summary_defaults_to_filename_when_absent() {
    let path = tmp_file(".log", b"hello");
    let (params, size) = prepare_attachment_params(AttachmentInput {
        file: &path,
        summary: None,
        content_type: None,
        is_patch: false,
        is_private: false,
        comment: None,
        flags: vec![],
    })
    .unwrap();
    assert_eq!(params.summary, params.file_name);
    assert_eq!(size, 5);
}

#[test]
fn empty_summary_falls_back_to_filename() {
    let path = tmp_file(".txt", b"x");
    let (params, _) = prepare_attachment_params(AttachmentInput {
        file: &path,
        summary: Some("   "),
        content_type: None,
        is_patch: false,
        is_private: false,
        comment: None,
        flags: vec![],
    })
    .unwrap();
    assert_eq!(params.summary, params.file_name);
}

#[test]
fn explicit_summary_and_content_type_win() {
    let path = tmp_file(".bin", b"data");
    let (params, _) = prepare_attachment_params(AttachmentInput {
        file: &path,
        summary: Some("boot trace"),
        content_type: Some("application/x-custom"),
        is_patch: false,
        is_private: true,
        comment: Some("see attached".into()),
        flags: vec![],
    })
    .unwrap();
    assert_eq!(params.summary, "boot trace");
    assert_eq!(params.content_type, "application/x-custom");
    assert!(params.is_private);
    assert_eq!(params.comment.as_deref(), Some("see attached"));
}

#[test]
fn patch_defaults_content_type_to_text_plain() {
    let path = tmp_file(".unknownext", b"diff");
    let (params, _) = prepare_attachment_params(AttachmentInput {
        file: &path,
        summary: None,
        content_type: None,
        is_patch: true,
        is_private: false,
        comment: None,
        flags: vec![],
    })
    .unwrap();
    assert_eq!(params.content_type, "text/plain");
    assert!(params.is_patch);
}

#[test]
fn missing_file_is_io_error_naming_the_path() {
    let err = prepare_attachment_params(AttachmentInput {
        file: Path::new("/nonexistent/bzr/does-not-exist.log"),
        summary: None,
        content_type: None,
        is_patch: false,
        is_private: false,
        comment: None,
        flags: vec![],
    })
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("does-not-exist.log"), "message was: {msg}");
}

#[test]
fn guess_content_type_known_and_unknown() {
    assert_eq!(guess_content_type("x.rs"), "text/plain");
    assert_eq!(guess_content_type("a.PNG"), "image/png");
    assert_eq!(guess_content_type("noext"), "application/octet-stream");
    assert_eq!(
        guess_content_type("a.unknownext"),
        "application/octet-stream"
    );
}
