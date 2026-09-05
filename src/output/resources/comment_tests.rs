#![expect(clippy::unwrap_used)]

use super::write_comments;
use crate::types::{Comment, OutputFormat};
use crate::validation::fields::FieldProjection;

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
        tags: vec![],
    }
}

fn capture(format: OutputFormat, comments: &[Comment]) -> String {
    capture_projected(format, comments, &FieldProjection::none())
}

fn capture_projected(
    format: OutputFormat,
    comments: &[Comment],
    projection: &FieldProjection,
) -> String {
    let mut buf = Vec::new();
    write_comments(comments, format, projection, &mut buf);
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
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
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
fn write_comments_table_renders_tags() {
    let mut tagged = make_comment(2, "body");
    tagged.tags = vec!["needs-info".into(), "reviewed".into()];
    let tagged_output = capture(OutputFormat::Table, &[tagged]);
    assert!(tagged_output.contains("2025-02-01T08:00:00Z)\n  Tags: needs-info, reviewed\n\n  body"));

    let untagged_output = capture(OutputFormat::Table, &[make_comment(2, "body")]);
    assert!(!untagged_output.contains("Tags:"));
}

#[test]
fn write_comments_table_escapes_tag_controls() {
    let mut comment = make_comment(2, "body");
    comment.tags = vec![
        "line\nbreak".into(),
        "carriage\rreturn".into(),
        "escape\u{1b}[2J".into(),
        "café".into(),
    ];

    let output = capture(OutputFormat::Table, &[comment]);

    assert!(output
        .contains("  Tags: line\\nbreak, carriage\\rreturn, escape\\u{1b}[2J, café\n\n  body"));
    assert!(!output.contains("line\nbreak"));
    assert!(!output.contains("carriage\rreturn"));
    assert!(!output.contains('\u{1b}'));
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
        tags: vec![],
    }];
    let output = capture(OutputFormat::Table, &comments);
    assert!(output.contains("unknown"));
    assert!(output.contains("héllo, wörld"));
    assert!(!output.contains("[PRIVATE]"));
}

#[test]
fn write_comments_json_one_comment_via_write() {
    let mut comment = make_comment(7, "json body");
    comment.tags = vec!["needs-info".into(), "reviewed".into()];
    let comments = vec![comment];
    let output = capture(OutputFormat::Json, &comments);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["count"], 7);
    assert_eq!(parsed[0]["text"], "json body");
    assert_eq!(parsed[0]["bug_id"], 42);
    assert_eq!(
        parsed[0]["tags"],
        serde_json::json!(["needs-info", "reviewed"])
    );
}

#[test]
fn write_comments_json_preserves_raw_tag_controls() {
    let raw_tags = vec![
        "line\nbreak".to_string(),
        "carriage\rreturn".to_string(),
        "escape\u{1b}[2J".to_string(),
        "café".to_string(),
    ];
    let mut comment = make_comment(7, "json body");
    comment.tags.clone_from(&raw_tags);

    let output = capture(OutputFormat::Json, &[comment]);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);

    assert_eq!(parsed[0]["tags"], serde_json::json!(raw_tags));
}

#[test]
fn write_comments_ndjson_includes_tags() {
    let mut comment = make_comment(7, "ndjson body");
    comment.tags = vec!["needs-info".into(), "reviewed".into()];
    let output = capture(OutputFormat::Ndjson, &[comment]);
    assert_eq!(output.lines().count(), 1);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(
        parsed["tags"],
        serde_json::json!(["needs-info", "reviewed"])
    );
}

#[test]
fn write_comments_json_projects_tags() {
    let mut comment = make_comment(7, "json body");
    comment.tags = vec!["needs-info".into(), "reviewed".into()];
    let projection = FieldProjection::resolve(Some("tags"), None, &["tags"]).unwrap();
    let output = capture_projected(OutputFormat::Json, &[comment], &projection);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        parsed,
        serde_json::json!([{"tags": ["needs-info", "reviewed"]}])
    );
}

#[test]
fn write_comments_ndjson_projects_tags() {
    let mut comment = make_comment(7, "ndjson body");
    comment.tags = vec!["needs-info".into(), "reviewed".into()];
    let projection = FieldProjection::resolve(Some("tags"), None, &["tags"]).unwrap();
    let output = capture_projected(OutputFormat::Ndjson, &[comment], &projection);
    assert_eq!(output.lines().count(), 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap(),
        serde_json::json!({"tags": ["needs-info", "reviewed"]})
    );
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

#[test]
fn bug_header_names_the_bug() {
    let mut out = Vec::new();
    crate::output::resources::comment::write_comment_bug_header(42, &mut out);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("Bug #42"),
        "header should name the bug: {text}"
    );
}
