#![expect(clippy::unwrap_used)]
//! Direct tests for the `bug update` execution entry points
//! ([`super::apply_checked`], [`super::apply_checked_connected`]): dry-run
//! short-circuits and the `--expect-unchanged-since` guard running before any
//! write.

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::commands::runtime::invocation::CommandContext;
use crate::test_helpers::{setup_test_env, CapturedIo};
use crate::types::bug::{CommentUpdate, UpdateBugParams};
use crate::types::comment::Comment;
use crate::types::OutputFormat;

use super::super::test_helpers::{
    forbid_put, mock_get_bug_lct, mock_put_bug_ok, received_put_count,
};
use super::{
    apply_checked, apply_checked_connected, find_latest_comment_id,
    warn_if_minor_update_unsupported, ApplyRequest,
};

fn comment_with(id: u64, count: Option<u64>) -> Comment {
    Comment {
        id,
        bug_id: None,
        text: None,
        creator: None,
        creation_time: None,
        count,
        is_private: None,
        attachment_id: None,
        tags: vec![],
    }
}

#[test]
fn find_latest_comment_id_prefers_highest_count() {
    let comments = vec![comment_with(50, Some(0)), comment_with(51, Some(1))];
    assert_eq!(find_latest_comment_id(&comments), Some(51));
}

#[test]
fn find_latest_comment_id_falls_back_to_last_when_count_is_absent() {
    let comments = vec![comment_with(70, None), comment_with(71, None)];
    assert_eq!(find_latest_comment_id(&comments), Some(71));
}

#[test]
fn find_latest_comment_id_none_when_empty() {
    assert_eq!(find_latest_comment_id(&[]), None);
}

/// Mount the GET the tagging sub-step uses to find the just-posted comment.
async fn mock_bug_comment(mock: &wiremock::MockServer, bug_id: u64, comment_id: u64, text: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{bug_id}/comment")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { bug_id.to_string(): { "comments": [{
                "id": comment_id, "bug_id": bug_id, "text": text,
                "creator": "user@test.com", "creation_time": "2025-01-01T00:00:00Z",
                "is_private": false, "count": 0
            }]}}
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn apply_checked_connected_dry_run_skips_expect_unchanged_get() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected get"))
        .expect(0)
        .mount(&mock)
        .await;
    forbid_put(&mock).await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: UpdateBugParams {
            status: Some("ASSIGNED".into()),
            ..Default::default()
        },
        expect_unchanged_since: Some("2026-06-19T12:00:00Z"),
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_dry_run(true);
    let mut io = CapturedIo::new();

    let result = apply_checked_connected(&client, request, &ctx, &mut io.writers()).await;

    assert!(result.is_ok(), "dry-run should not re-read: {result:?}");
    assert_eq!(received_put_count(&mock).await, 0);
    let requests = mock.received_requests().await.unwrap();
    assert!(
        requests.is_empty(),
        "dry-run should skip optimistic GET and PUT, got {requests:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([42]));
}

#[tokio::test]
async fn apply_checked_dry_run_skips_connection_setup() {
    let tmp = tempfile::tempdir().unwrap();
    let request = ApplyRequest {
        ids: vec![42],
        params: UpdateBugParams {
            status: Some("ASSIGNED".into()),
            ..Default::default()
        },
        expect_unchanged_since: Some("2026-06-19T12:00:00Z"),
    };
    let ctx = CommandContext::new(Some("missing"), OutputFormat::Json, None)
        .with_config_path_override(Some(tmp.path().join("missing-config.toml")))
        .with_dry_run(true);
    let mut io = CapturedIo::new();

    let result = apply_checked(request, &ctx, &mut io.writers()).await;

    assert!(
        result.is_ok(),
        "dry-run should render without loading config or connecting: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([42]));
    assert_eq!(parsed["changes"]["status"], "ASSIGNED");
}

#[tokio::test]
async fn apply_checked_connected_runs_guard_before_any_write() {
    // The guard re-reads bug 42 and sees a newer last_change_time than the
    // caller expected: the write must be refused before any PUT.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "2026-06-19T12:00:00Z").await;
    forbid_put(&mock).await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: UpdateBugParams {
            status: Some("ASSIGNED".into()),
            ..Default::default()
        },
        expect_unchanged_since: Some("2026-06-19T10:00:00Z"),
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = CapturedIo::new();

    let result = apply_checked_connected(&client, request, &ctx, &mut io.writers()).await;

    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::MidAirCollision { id: 42, .. }),
        "expected a collision on bug 42, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn apply_checked_connected_writes_when_guard_passes() {
    // The guard re-read matches the expected timestamp, so the write proceeds.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "2026-06-19T12:00:00Z").await;
    mock_put_bug_ok(&mock, 42).await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: UpdateBugParams {
            status: Some("ASSIGNED".into()),
            ..Default::default()
        },
        expect_unchanged_since: Some("2026-06-19T12:00:00Z"),
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = CapturedIo::new();

    let result = apply_checked_connected(&client, request, &ctx, &mut io.writers()).await;

    assert!(result.is_ok(), "matching guard should write: {result:?}");
    assert_eq!(received_put_count(&mock).await, 1);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

fn params_with_comment_tags(body: &str, tags: &[&str]) -> UpdateBugParams {
    UpdateBugParams {
        comment: Some(CommentUpdate {
            body: body.into(),
            is_private: false,
        }),
        comment_tags: tags.iter().map(|t| (*t).to_string()).collect(),
        ..Default::default()
    }
}

#[tokio::test]
async fn single_update_tags_the_posted_comment_after_a_bug_update_that_ignores_the_field() {
    // Bug.update's `comment_tags` parameter is not reliably honored (issue
    // #672), so this must tag the comment via a follow-up GET + PUT
    // regardless of whether the update response acknowledged the tags.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;
    mock_bug_comment(&mock, 42, 900, "tagged comment").await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/900/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["triaged"])))
        .expect(1)
        .mount(&mock)
        .await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: params_with_comment_tags("tagged comment", &["triaged"]),
        expect_unchanged_since: None,
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = CapturedIo::new();

    let result = apply_checked_connected(&client, request, &ctx, &mut io.writers()).await;

    assert!(result.is_ok(), "tagging should succeed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn single_update_tag_lookup_failure_fails_the_update_and_warns_against_retry() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;
    // The comment GET returns nothing at all: there is no comment to tag.
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": {"42": {"comments": []}}})),
        )
        .mount(&mock)
        .await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: params_with_comment_tags("tagged comment", &["triaged"]),
        expect_unchanged_since: None,
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = CapturedIo::new();

    let err = apply_checked_connected(&client, request, &ctx, &mut io.writers())
        .await
        .unwrap_err();

    assert!(matches!(err, crate::error::BzrError::NotFound { .. }));
    assert!(
        io.err_str().contains("do not retry") || io.err_str().contains("Do not retry"),
        "stderr should warn against retrying with the same comment: {}",
        io.err_str()
    );
}

#[tokio::test]
async fn single_update_tags_the_comment_with_the_highest_count_ignoring_stale_text() {
    // Regression: Bugzilla does not always round-trip comment text
    // byte-for-byte (e.g. trailing whitespace stripped), so matching by
    // text is unreliable. The tagging step must select the comment with the
    // highest `count` regardless of what its text looks like.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "42": { "comments": [
                {"id": 800, "bug_id": 42, "text": "an earlier comment",
                 "creator": "user@test.com", "creation_time": "2025-01-01T00:00:00Z",
                 "is_private": false, "count": 0},
                {"id": 900, "bug_id": 42, "text": "tagged comment",
                 "creator": "user@test.com", "creation_time": "2025-01-01T00:00:01Z",
                 "is_private": false, "count": 1}
            ]}}
        })))
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/900/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["triaged"])))
        .expect(1)
        .mount(&mock)
        .await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![42],
        params: params_with_comment_tags("tagged comment", &["triaged"]),
        expect_unchanged_since: None,
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = CapturedIo::new();

    let result = apply_checked_connected(&client, request, &ctx, &mut io.writers()).await;

    assert!(result.is_ok(), "tagging should succeed: {result:?}");
}

#[tokio::test]
async fn batch_update_tag_failure_reports_that_id_as_failed() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 1).await;
    mock_put_bug_ok(&mock, 2).await;
    mock_bug_comment(&mock, 1, 901, "tagged comment").await;
    // Bug 2's comment lookup finds nothing matching: its tag step fails.
    Mock::given(method("GET"))
        .and(path("/rest/bug/2/comment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": {"2": {"comments": []}}})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/901/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["triaged"])))
        .mount(&mock)
        .await;
    let client = crate::client::test_helpers::test_client(&mock.uri());
    let request = ApplyRequest {
        ids: vec![1, 2],
        params: params_with_comment_tags("tagged comment", &["triaged"]),
        expect_unchanged_since: None,
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_assume_yes(true);
    let mut io = CapturedIo::new();

    let err = apply_checked_connected(&client, request, &ctx, &mut io.writers())
        .await
        .unwrap_err();

    assert_eq!(err.exit_code(), 11);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["succeeded"], serde_json::json!([1]));
    assert_eq!(parsed["failed"][0]["id"], 2);
    // The field update and comment for bug 2 already succeeded; only tagging
    // failed. `step` distinguishes that from a fully-failed update so a
    // caller does not retry and duplicate the comment.
    assert_eq!(parsed["failed"][0]["step"], "comment_tags");
    assert!(
        io.err_str().contains("do not retry") || io.err_str().contains("Do not retry"),
        "stderr should warn against retrying with the same comment: {}",
        io.err_str()
    );
}

fn config_with_server_version(tmp: &tempfile::TempDir, version: &str) -> std::path::PathBuf {
    let contents = format!(
        r#"
default_server = "test"

[servers.test]
url = "http://example.invalid"
server_version = "{version}"
"#
    );
    crate::test_helpers::write_config_to(tmp, &contents)
}

fn ctx_with_config(config_path: &std::path::Path) -> CommandContext {
    CommandContext::new(None, OutputFormat::Json, None)
        .with_config_path_override(Some(config_path.to_path_buf()))
}

#[test]
fn warn_if_minor_update_unsupported_warns_below_the_floor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_with_server_version(&tmp, "5.0.6");
    let ctx = ctx_with_config(&config_path);
    let mut io = CapturedIo::new();

    warn_if_minor_update_unsupported(&ctx, true, &mut io.writers());

    assert!(
        io.err_str().contains("5.0.6") && io.err_str().contains("--minor-update"),
        "stderr: {}",
        io.err_str()
    );
}

#[test]
fn warn_if_minor_update_unsupported_silent_at_or_above_the_floor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_with_server_version(&tmp, "5.3.3+");
    let ctx = ctx_with_config(&config_path);
    let mut io = CapturedIo::new();

    warn_if_minor_update_unsupported(&ctx, true, &mut io.writers());

    assert_eq!(io.err_str(), "", "should not warn at/above the floor");
}

#[test]
fn warn_if_minor_update_unsupported_silent_when_not_requested() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_with_server_version(&tmp, "5.0.6");
    let ctx = ctx_with_config(&config_path);
    let mut io = CapturedIo::new();

    warn_if_minor_update_unsupported(&ctx, false, &mut io.writers());

    assert_eq!(io.err_str(), "");
}

#[test]
fn warn_if_minor_update_unsupported_silent_when_version_unknown() {
    let tmp = tempfile::TempDir::new().unwrap();
    let contents = r#"
default_server = "test"

[servers.test]
url = "http://example.invalid"
"#;
    let config_path = crate::test_helpers::write_config_to(&tmp, contents);
    let ctx = ctx_with_config(&config_path);
    let mut io = CapturedIo::new();

    warn_if_minor_update_unsupported(&ctx, true, &mut io.writers());

    assert_eq!(
        io.err_str(),
        "",
        "no cached version means nothing to warn about"
    );
}

#[test]
fn warn_if_minor_update_unsupported_silent_for_inline_server() {
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_inline_server(Some(
        crate::commands::runtime::invocation::InlineServer {
            url: "http://example.invalid".into(),
            api_key_env: None,
            email: None,
            tls: crate::commands::runtime::invocation::InlineTlsOptions::default(),
        },
    ));
    let mut io = CapturedIo::new();

    warn_if_minor_update_unsupported(&ctx, true, &mut io.writers());

    assert_eq!(
        io.err_str(),
        "",
        "an inline connection has no relevant cached version to check"
    );
}
