#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, CommentArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn ok_put(id: u64) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"bugs": [{"id": id, "changes": {}}]}))
}

/// Mount a PUT mock on `/rest/bug/{id}` asserting the exact JSON body, then run
/// `execute` for `action` and assert success.
async fn run_verb_expecting_body(action: BugAction, id: u64, body: serde_json::Value) {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path(format!("/rest/bug/{id}")))
        .and(body_json(body))
        .respond_with(ok_put(id))
        .expect(1)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;
    assert!(result.is_ok(), "verb failed: {:?}", result.err());
}

#[tokio::test]
async fn resolve_dry_run_makes_no_write() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // A verb PUT must never fire under --dry-run; the connect probe is a HEAD.
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;
    crate::commands::dry_run::set(true);

    let action = BugAction::Resolve {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        comment: CommentArgs::default(),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;
    let output = io.out_str().to_string();
    crate::commands::dry_run::set(false);

    assert!(result.is_ok(), "dry-run resolve failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([5]));
    assert_eq!(parsed["changes"]["status"], "RESOLVED");
    assert_eq!(parsed["changes"]["resolution"], "FIXED");
}

#[tokio::test]
async fn resolve_defaults_to_fixed() {
    let action = BugAction::Resolve {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        comment: CommentArgs::default(),
    };
    run_verb_expecting_body(
        action,
        5,
        serde_json::json!({"status": "RESOLVED", "resolution": "FIXED"}),
    )
    .await;
}

#[tokio::test]
async fn resolve_with_as_override() {
    let action = BugAction::Resolve {
        ids: vec![7],
        as_resolution: "WONTFIX".into(),
        comment: CommentArgs::default(),
    };
    run_verb_expecting_body(
        action,
        7,
        serde_json::json!({"status": "RESOLVED", "resolution": "WONTFIX"}),
    )
    .await;
}

#[tokio::test]
async fn close_without_resolution_preserves_existing() {
    let action = BugAction::Close {
        ids: vec![9],
        as_resolution: None,
        comment: CommentArgs::default(),
    };
    // No resolution key — the server keeps any existing resolution.
    run_verb_expecting_body(action, 9, serde_json::json!({"status": "CLOSED"})).await;
}

#[tokio::test]
async fn close_with_as_sets_resolution() {
    let action = BugAction::Close {
        ids: vec![9],
        as_resolution: Some("WONTFIX".into()),
        comment: CommentArgs::default(),
    };
    run_verb_expecting_body(
        action,
        9,
        serde_json::json!({"status": "CLOSED", "resolution": "WONTFIX"}),
    )
    .await;
}

#[tokio::test]
async fn reopen_sends_reopened_status() {
    let action = BugAction::Reopen {
        ids: vec![3],
        comment: CommentArgs::default(),
    };
    run_verb_expecting_body(action, 3, serde_json::json!({"status": "REOPENED"})).await;
}

#[tokio::test]
async fn dup_sends_dupe_of() {
    let action = BugAction::Dup {
        id: 12,
        target: 100,
        comment: CommentArgs::default(),
    };
    run_verb_expecting_body(action, 12, serde_json::json!({"dupe_of": 100})).await;
}

#[tokio::test]
async fn resolve_posts_comment_atomically() {
    let action = BugAction::Resolve {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        comment: CommentArgs {
            comment: Some("done in 9.1".into()),
            comment_file: None,
            comment_private: false,
        },
    };
    run_verb_expecting_body(
        action,
        5,
        // is_private is omitted when false (skip_serializing_if).
        serde_json::json!({
            "status": "RESOLVED",
            "resolution": "FIXED",
            "comment": {"body": "done in 9.1"}
        }),
    )
    .await;
}

#[tokio::test]
async fn resolve_batch_updates_each_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    for id in [1_u64, 2] {
        Mock::given(method("PUT"))
            .and(path(format!("/rest/bug/{id}")))
            .and(body_json(
                serde_json::json!({"status": "RESOLVED", "resolution": "FIXED"}),
            ))
            .respond_with(ok_put(id))
            .expect(1)
            .mount(&mock)
            .await;
    }

    let action = BugAction::Resolve {
        ids: vec![1, 2],
        as_resolution: "FIXED".into(),
        comment: CommentArgs::default(),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;
    assert!(result.is_ok(), "batch resolve failed: {:?}", result.err());
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    // Batch JSON shape from update_batch / BatchResult.
    assert_eq!(parsed["succeeded"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn close_private_comment_without_body_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = BugAction::Close {
        ids: vec![5],
        as_resolution: None,
        comment: CommentArgs {
            comment: None,
            comment_file: None,
            comment_private: true,
        },
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(m) if m.contains("--comment-private")),
        "got {err:?}"
    );
}
