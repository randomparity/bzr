#![expect(clippy::unwrap_used)]

use super::print_comments;
use crate::types::{Comment, OutputFormat};

fn make_comment(count: u64, text: &str) -> Comment {
    Comment {
        id: count + 100,
        bug_id: 42,
        text: text.into(),
        creator: Some("commenter@example.com".into()),
        creation_time: Some("2025-02-01T08:00:00Z".into()),
        count,
        is_private: false,
    }
}

#[test]
fn print_comments_json_empty() {
    let comments: Vec<Comment> = vec![];
    let json = serde_json::to_string_pretty(&comments).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn print_comments_json_one_comment() {
    let comments = vec![make_comment(0, "First comment text")];
    let json = serde_json::to_string_pretty(&comments).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["text"], "First comment text");
    assert_eq!(parsed[0]["count"], 0);
    assert_eq!(parsed[0]["creator"], "commenter@example.com");
}

#[test]
fn print_comments_json_private_flag() {
    let mut comment = make_comment(1, "secret");
    comment.is_private = true;
    let json = serde_json::to_string(&comment).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["is_private"], true);
}

// ── capture_stdout-based formatter tests ─────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn print_comments_table_empty_says_no_comments() {
    let _lock = crate::ENV_LOCK.lock().await;
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_comments(&[], OutputFormat::Table);
    })
    .await;
    assert!(output.contains("No comments."));
}

#[cfg(unix)]
#[tokio::test]
async fn print_comments_json_empty_renders_empty_array() {
    let _lock = crate::ENV_LOCK.lock().await;
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_comments(&[], OutputFormat::Json);
    })
    .await;
    let parsed = crate::test_helpers::extract_json(&output);
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn print_comments_table_renders_comment_fields() {
    let _lock = crate::ENV_LOCK.lock().await;
    let mut c = make_comment(2, "Line one\nLine two");
    c.is_private = true;
    let comments = vec![c];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_comments(&comments, OutputFormat::Table);
    })
    .await;
    assert!(output.contains("Comment"));
    assert!(output.contains("#2"));
    assert!(output.contains("commenter@example.com"));
    assert!(output.contains("2025-02-01T08:00:00Z"));
    assert!(output.contains("[PRIVATE]"));
    assert!(output.contains("Line one"));
    assert!(output.contains("Line two"));
    // Separator row
    assert!(output.contains('─'));
}

#[cfg(unix)]
#[tokio::test]
async fn print_comments_table_handles_missing_creator_and_unicode() {
    let _lock = crate::ENV_LOCK.lock().await;
    let comments = vec![Comment {
        id: 1,
        bug_id: 42,
        text: "héllo, wörld".into(),
        creator: None,
        creation_time: None,
        count: 0,
        is_private: false,
    }];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_comments(&comments, OutputFormat::Table);
    })
    .await;
    // Falls back to "unknown" for missing creator and "" for missing time
    assert!(output.contains("unknown"));
    assert!(output.contains("héllo, wörld"));
    // Private flag absent — verify [PRIVATE] is NOT present
    assert!(!output.contains("[PRIVATE]"));
}

#[cfg(unix)]
#[tokio::test]
async fn print_comments_json_one_comment_via_print() {
    let _lock = crate::ENV_LOCK.lock().await;
    let comments = vec![make_comment(7, "json body")];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_comments(&comments, OutputFormat::Json);
    })
    .await;
    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["count"], 7);
    assert_eq!(parsed[0]["text"], "json body");
    assert_eq!(parsed[0]["bug_id"], 42);
}
