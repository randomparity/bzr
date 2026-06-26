#![expect(clippy::unwrap_used)]
//! Direct tests for the `bug update` output formatters: the batch result
//! envelope ([`super::write_batch_result`]) in table and JSON form, and the
//! dry-run preview ([`super::write_update_dry_run`]).

use crate::output::result_types::{BatchFailure, BatchResult};
use crate::test_helpers::CapturedIo;
use crate::types::bug::{CommentUpdate, UpdateBugParams};
use crate::types::OutputFormat;

use super::{write_batch_result, write_update_dry_run};

#[test]
fn write_batch_result_table_prints_successes_and_failures() {
    let batch = BatchResult::new(
        vec![1],
        vec![BatchFailure {
            id: 2,
            error: "boom".into(),
        }],
    );
    let mut io = CapturedIo::new();

    write_batch_result(&batch, OutputFormat::Table, true, &mut io.writers());

    assert_eq!(io.out_str(), "Updated bugs: #1 (with comment)\n");
    assert_eq!(io.err_str(), "Failed to update bug #2: boom\n");
}

#[test]
fn write_batch_result_json_emits_batch_result_shape() {
    let batch = BatchResult::new(
        vec![7],
        vec![BatchFailure {
            id: 8,
            error: "nope".into(),
        }],
    );
    let mut io = CapturedIo::new();

    write_batch_result(&batch, OutputFormat::Json, false, &mut io.writers());

    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "bug");
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["succeeded"], serde_json::json!([7]));
    assert_eq!(parsed["failed"][0]["id"], 8);
    assert_eq!(parsed["failed"][0]["error"], "nope");
    assert!(io.err_str().is_empty());
}

#[test]
fn write_update_dry_run_json_marks_payload_and_lists_ids() {
    let params = UpdateBugParams {
        status: Some("ASSIGNED".into()),
        ..Default::default()
    };
    let mut io = CapturedIo::new();

    write_update_dry_run(&[7, 8], &params, OutputFormat::Json, &mut io.writers());

    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["resource"], "bug");
    assert_eq!(parsed["ids"], serde_json::json!([7, 8]));
    assert_eq!(parsed["changes"]["status"], "ASSIGNED");
}

#[test]
fn write_update_dry_run_table_prints_human_preview_with_comment_suffix() {
    let params = UpdateBugParams {
        status: Some("ASSIGNED".into()),
        comment: Some(CommentUpdate {
            body: "hi".into(),
            is_private: false,
        }),
        ..Default::default()
    };
    let mut io = CapturedIo::new();

    write_update_dry_run(&[7], &params, OutputFormat::Table, &mut io.writers());

    let out = io.out_str();
    assert!(
        out.contains("Dry run"),
        "expected human preview, got: {out}"
    );
    assert!(out.contains("#7"), "expected bug id, got: {out}");
    assert!(
        out.contains("(with comment)"),
        "expected comment suffix, got: {out}"
    );
}
