#![expect(clippy::unwrap_used)]

use super::write_comments;
use crate::types::{Comment, OutputFormat};

fn make_comment(count: u64, text: &str) -> Comment {
    Comment {
        id: count + 100,
        bug_id: Some(42),
        text: Some(text.into()),
        creator: Some("commenter@example.com".into()),
        creation_time: Some("2025-02-01T08:00:00Z".into()),
        count: Some(count),
        is_private: Some(false),
        attachment_id: None,
    }
}

fn capture(format: OutputFormat, comments: &[Comment]) -> String {
    let mut buf = Vec::new();
    write_comments(comments, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_comments_json_empty() {
    let comments: Vec<Comment> = vec![];
    let json = serde_json::to_string_pretty(&comments).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_comments_json_one_comment() {
    let comments = vec![make_comment(0, "First comment text")];
    let json = serde_json::to_string_pretty(&comments).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["text"], "First comment text");
    assert_eq!(parsed[0]["count"], 0);
    assert_eq!(parsed[0]["creator"], "commenter@example.com");
}

#[test]
fn write_comments_json_private_flag() {
    let mut comment = make_comment(1, "secret");
    comment.is_private = Some(true);
    let json = serde_json::to_string(&comment).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["is_private"], true);
}

#[test]
fn write_comments_table_empty_says_no_comments() {
    let output = capture(OutputFormat::Table, &[]);
    assert!(output.contains("No comments."));
}

#[test]
fn write_comments_json_empty_renders_empty_array() {
    let output = capture(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_comments_table_renders_comment_fields() {
    let mut c = make_comment(2, "Line one\nLine two");
    c.is_private = Some(true);
    let output = capture(OutputFormat::Table, &[c]);
    assert!(output.contains("Comment"));
    assert!(output.contains("#2"));
    assert!(output.contains("commenter@example.com"));
    assert!(output.contains("2025-02-01T08:00:00Z"));
    assert!(output.contains("[PRIVATE]"));
    assert!(output.contains("Line one"));
    assert!(output.contains("Line two"));
    assert!(output.contains('─'));
}

#[test]
fn write_comments_table_handles_missing_creator_and_unicode() {
    let comments = vec![Comment {
        id: 1,
        bug_id: Some(42),
        text: Some("héllo, wörld".into()),
        creator: None,
        creation_time: None,
        count: Some(0),
        is_private: Some(false),
        attachment_id: None,
    }];
    let output = capture(OutputFormat::Table, &comments);
    assert!(output.contains("unknown"));
    assert!(output.contains("héllo, wörld"));
    assert!(!output.contains("[PRIVATE]"));
}

#[test]
fn write_comments_json_one_comment_via_write() {
    let comments = vec![make_comment(7, "json body")];
    let output = capture(OutputFormat::Json, &comments);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["count"], 7);
    assert_eq!(parsed[0]["text"], "json body");
    assert_eq!(parsed[0]["bug_id"], 42);
}

#[test]
fn write_comments_table_does_not_emit_ansi_when_writing_to_buffer() {
    // colored disables ANSI for non-TTY writers; Vec<u8> is not a TTY.
    let comments = vec![make_comment(3, "body")];
    let output = capture(OutputFormat::Table, &comments);
    assert!(
        !output.contains('\x1b'),
        "expected no ANSI escapes when writing to Vec<u8>: {output:?}",
    );
}
