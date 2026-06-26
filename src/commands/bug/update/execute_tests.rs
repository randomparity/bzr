#![expect(clippy::unwrap_used)]
//! Direct tests for the `bug update` execution entry points
//! ([`super::apply_checked`], [`super::apply_checked_connected`]): dry-run
//! short-circuits and the `--expect-unchanged-since` guard running before any
//! write.

use wiremock::matchers::method;
use wiremock::{Mock, ResponseTemplate};

use crate::commands::runtime::context::CommandContext;
use crate::test_helpers::{setup_test_env, CapturedIo};
use crate::types::bug::UpdateBugParams;
use crate::types::OutputFormat;

use super::super::test_helpers::{
    forbid_put, mock_get_bug_lct, mock_put_bug_ok, received_put_count,
};
use super::{apply_checked, apply_checked_connected, ApplyRequest};

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
