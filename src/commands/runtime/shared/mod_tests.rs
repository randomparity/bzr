#![expect(clippy::unwrap_used, clippy::panic)]

use crate::error::BzrError;

use super::{
    classify_body_source, materialize_body_source, materialize_comment_body,
    materialize_optional_comment_body, BodySource, CommentBodyRequirement,
};

fn fallback_comment_body() -> crate::error::Result<String> {
    let mut source = &b"fallback body"[..];
    let mut body = String::new();
    std::io::Read::read_to_string(&mut source, &mut body)?;
    Ok(body)
}

#[test]
fn classify_inline_literal() {
    let got = classify_body_source(Some("hello"), None, "--body", "--body-file").unwrap();
    assert_eq!(got, BodySource::Literal("hello".to_string()));
}

#[test]
fn classify_inline_dash_is_stdin() {
    let got = classify_body_source(Some("-"), None, "--body", "--body-file").unwrap();
    assert_eq!(
        got,
        BodySource::Stdin {
            flag: "--body".to_string()
        }
    );
}

#[test]
fn classify_file_path() {
    let got = classify_body_source(
        None,
        Some(std::path::Path::new("/tmp/x")),
        "--body",
        "--body-file",
    )
    .unwrap();
    assert_eq!(got, BodySource::File(std::path::PathBuf::from("/tmp/x")));
}

#[test]
fn classify_file_dash_is_stdin() {
    let got = classify_body_source(
        None,
        Some(std::path::Path::new("-")),
        "--body",
        "--body-file",
    )
    .unwrap();
    assert_eq!(
        got,
        BodySource::Stdin {
            flag: "--body-file".to_string()
        }
    );
}

#[test]
fn classify_none() {
    let got = classify_body_source(None, None, "--body", "--body-file").unwrap();
    assert_eq!(got, BodySource::None);
}

#[test]
fn classify_both_is_mutually_exclusive_error() {
    let err = classify_body_source(
        Some("x"),
        Some(std::path::Path::new("/tmp/x")),
        "--comment",
        "--comment-file",
    )
    .unwrap_err();
    match err {
        BzrError::InputValidation { message: msg, .. } => {
            assert!(msg.contains("--comment"), "names inline flag: {msg}");
            assert!(msg.contains("--comment-file"), "names file flag: {msg}");
        }
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn read_to_string_from_reads_bytes() {
    let mut src = &b"piped body\n"[..];
    let got = super::read_to_string_from(&mut src, "read test body").unwrap();
    assert_eq!(got, "piped body\n");
}

#[test]
fn read_to_string_from_rejects_non_utf8() {
    let mut src = &[0xff, 0xfe][..];
    let err = super::read_to_string_from(&mut src, "read test body").unwrap_err();
    match err {
        BzrError::Io(e) => assert!(
            e.to_string().contains("read test body"),
            "context missing: {e}"
        ),
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn materialize_literal_passes_through() {
    let got = materialize_body_source(BodySource::Literal("hi".into()), "--body-file").unwrap();
    assert_eq!(got, Some("hi".to_string()));
}

#[test]
fn materialize_none_is_none() {
    let got = materialize_body_source(BodySource::None, "--body-file").unwrap();
    assert_eq!(got, None);
}

#[test]
fn materialize_missing_file_is_input_validation() {
    let err = materialize_body_source(
        BodySource::File(std::path::PathBuf::from("/no/such/file/here")),
        "--body-file",
    )
    .unwrap_err();
    match err {
        BzrError::InputValidation { message: msg, .. } => {
            assert!(msg.contains("--body-file"), "{msg}");
        }
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn materialize_comment_body_uses_required_fallback() {
    let got = materialize_comment_body(
        BodySource::None,
        "--body-file",
        CommentBodyRequirement::RequiredWithFallback(fallback_comment_body),
    )
    .unwrap();
    assert_eq!(got, Some("fallback body".to_string()));
}

#[test]
fn materialize_comment_body_accepts_literal() {
    let got = materialize_comment_body(
        BodySource::Literal("hi".into()),
        "--comment-file",
        CommentBodyRequirement::Optional,
    )
    .unwrap();
    assert_eq!(got, Some("hi".to_string()));
}

#[test]
fn materialize_comment_body_allows_absent_optional_body() {
    let got = materialize_comment_body(
        BodySource::None,
        "--comment-file",
        CommentBodyRequirement::Optional,
    )
    .unwrap();
    assert_eq!(got, None);
}

#[test]
fn materialize_optional_comment_body_accepts_comment_flag() {
    let got = materialize_optional_comment_body(Some("hi"), None, false).unwrap();
    assert_eq!(got, Some("hi".to_string()));
}

#[test]
fn materialize_optional_comment_body_rejects_private_without_body() {
    let err = materialize_optional_comment_body(None, None, true).unwrap_err();
    assert!(matches!(
        err,
        BzrError::InputValidation { message: ref msg, .. }
            if msg.contains("--comment-private") && msg.contains("--comment-file")
    ));
}

#[test]
fn materialize_comment_body_rejects_required_private_body() {
    let err = materialize_comment_body(
        BodySource::None,
        "--comment-file",
        CommentBodyRequirement::PrivateRequiresBody,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        BzrError::InputValidation { message: ref msg, .. }
            if msg.contains("--comment-private") && msg.contains("--comment-file")
    ));
}

#[test]
fn materialize_comment_body_rejects_whitespace() {
    let err = materialize_comment_body(
        BodySource::Literal(" \n\t".into()),
        "--comment-file",
        CommentBodyRequirement::Optional,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        BzrError::InputValidation { message: ref msg, .. } if msg.contains("empty comment")
    ));
}
