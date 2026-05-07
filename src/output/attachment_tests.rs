#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::{Attachment, OutputFormat};

fn make_attachment(id: u64, summary: &str) -> Attachment {
    Attachment {
        id,
        bug_id: 42,
        file_name: format!("file_{id}.patch"),
        summary: summary.into(),
        content_type: "text/plain".into(),
        creator: Some("author@example.com".into()),
        creation_time: Some("2025-03-01T09:00:00Z".into()),
        last_change_time: Some("2025-03-02T10:00:00Z".into()),
        size: 1234,
        is_obsolete: false,
        is_private: false,
        is_patch: false,
        data: None,
    }
}

#[test]
fn print_attachments_json_empty() {
    let attachments: Vec<Attachment> = vec![];
    let json = serde_json::to_string_pretty(&attachments).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn print_attachments_json_one_attachment() {
    let attachments = vec![make_attachment(10, "Fix patch")];
    let json = serde_json::to_string_pretty(&attachments).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["id"], 10);
    assert_eq!(parsed[0]["summary"], "Fix patch");
    assert_eq!(parsed[0]["file_name"], "file_10.patch");
    assert_eq!(parsed[0]["content_type"], "text/plain");
    assert_eq!(parsed[0]["size"], 1234);
}

#[test]
fn attachment_text_format_fields() {
    let att = make_attachment(10, "Fix patch");
    assert_eq!(att.file_name, "file_10.patch");
    assert_eq!(att.content_type, "text/plain");
    assert_eq!(att.size, 1234);
    assert_eq!(att.creator.as_deref(), Some("author@example.com"));
    assert!(!att.is_obsolete);
    assert!(!att.is_private);
}

#[test]
fn print_attachments_json_obsolete_and_private() {
    let mut att = make_attachment(11, "Old patch");
    att.is_obsolete = true;
    att.is_private = true;
    let json = serde_json::to_string(&att).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["is_obsolete"], true);
    assert_eq!(parsed["is_private"], true);
}

// ── capture_stdout-based formatter tests ─────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_table_empty_says_no_attachments() {
    let _lock = crate::ENV_LOCK.lock().await;
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&[], OutputFormat::Table);
    })
    .await;
    assert!(output.contains("No attachments."));
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_json_empty_renders_empty_array() {
    let _lock = crate::ENV_LOCK.lock().await;
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&[], OutputFormat::Json);
    })
    .await;
    let parsed = crate::test_helpers::extract_json(&output);
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_table_renders_all_fields() {
    let _lock = crate::ENV_LOCK.lock().await;
    let mut att = make_attachment(7, "Helpful patch");
    att.is_obsolete = true;
    att.is_private = true;
    let attachments = vec![att];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&attachments, OutputFormat::Table);
    })
    .await;
    assert!(output.contains("Attachment"));
    assert!(output.contains("#7"));
    assert!(output.contains("Helpful patch"));
    assert!(output.contains("[OBSOLETE]"));
    assert!(output.contains("[PRIVATE]"));
    assert!(output.contains("file_7.patch"));
    assert!(output.contains("text/plain"));
    assert!(output.contains("1234 bytes"));
    assert!(output.contains("Creator"));
    assert!(output.contains("author@example.com"));
    assert!(output.contains("Created"));
    assert!(output.contains("2025-03-01T09:00:00Z"));
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_table_missing_optional_fields_render_dash() {
    let _lock = crate::ENV_LOCK.lock().await;
    let attachments = vec![Attachment {
        id: 8,
        bug_id: 42,
        file_name: "patché.txt".into(),
        summary: "Unicode summary — é".into(),
        content_type: "text/plain".into(),
        creator: None,
        creation_time: None,
        last_change_time: None,
        size: 0,
        is_obsolete: false,
        is_private: false,
        is_patch: false,
        data: None,
    }];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&attachments, OutputFormat::Table);
    })
    .await;
    assert!(output.contains("Unicode summary — é"));
    assert!(output.contains("patché.txt"));
    // Missing creator/created render as "-" via print_optional_field's
    // "  {label:<12}  {value}" format. Anchor to the trailing field
    // value so the assertion fails on rendering bugs, not table-border
    // changes.
    assert!(
        output.contains("Creator       -"),
        "expected dashed Creator field, got: {output}"
    );
    assert!(
        output.contains("Created       -"),
        "expected dashed Created field, got: {output}"
    );
    // Neither flag is set
    assert!(!output.contains("[OBSOLETE]"));
    assert!(!output.contains("[PRIVATE]"));
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_table_renders_patch_tag() {
    let _lock = crate::ENV_LOCK.lock().await;
    let mut att = make_attachment(12, "Patch attachment");
    att.is_patch = true;
    let attachments = vec![att];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&attachments, OutputFormat::Table);
    })
    .await;
    assert!(output.contains("Attachment"));
    assert!(output.contains("#12"));
    assert!(output.contains("Patch attachment"));
    assert!(output.contains("[PATCH]"));
    // is_patch alone should not enable the other tags.
    assert!(!output.contains("[OBSOLETE]"));
    assert!(!output.contains("[PRIVATE]"));
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_table_patch_tag_precedes_obsolete_and_private() {
    let _lock = crate::ENV_LOCK.lock().await;
    let mut att = make_attachment(13, "All flags");
    att.is_patch = true;
    att.is_obsolete = true;
    att.is_private = true;
    let attachments = vec![att];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&attachments, OutputFormat::Table);
    })
    .await;
    let patch_idx = output.find("[PATCH]").expect("[PATCH] tag should render");
    let obsolete_idx = output
        .find("[OBSOLETE]")
        .expect("[OBSOLETE] tag should render");
    let private_idx = output
        .find("[PRIVATE]")
        .expect("[PRIVATE] tag should render");
    assert!(patch_idx < obsolete_idx);
    assert!(obsolete_idx < private_idx);
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachments_json_one_via_print() {
    let _lock = crate::ENV_LOCK.lock().await;
    let attachments = vec![make_attachment(99, "Json patch")];
    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_attachments(&attachments, OutputFormat::Json);
    })
    .await;
    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["id"], 99);
    assert_eq!(parsed[0]["summary"], "Json patch");
    assert_eq!(parsed[0]["file_name"], "file_99.patch");
}

#[test]
fn attachment_batch_result_json_shape() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![BugDownloadResult {
            bug_id: 12345,
            status: TargetStatus::Ok,
            files: vec![DownloadedFile {
                attachment_id: 9876,
                path: "./attachments/12345/9876.patch.diff".into(),
                bytes: 4096,
            }],
            error: None,
        }],
        attachment_results: vec![],
        summary: BatchSummary {
            succeeded: 1,
            failed: 0,
            total_bytes: 4096,
        },
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["out_dir"], "./attachments");
    assert_eq!(json["bug_results"][0]["bug_id"], 12345);
    assert_eq!(json["bug_results"][0]["status"], "ok");
    assert_eq!(json["bug_results"][0]["files"][0]["attachment_id"], 9876);
    assert_eq!(json["bug_results"][0]["files"][0]["bytes"], 4096);
    assert_eq!(json["summary"]["succeeded"], 1);
    assert_eq!(json["summary"]["total_bytes"], 4096);
}

#[test]
fn attachment_batch_result_omits_empty_optional_fields() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![BugDownloadResult {
            bug_id: 12345,
            status: TargetStatus::Ok,
            files: vec![],
            error: None,
        }],
        attachment_results: vec![AttachmentDownloadResult {
            attachment_id: 999,
            status: TargetStatus::Error,
            bug_id: None,
            path: None,
            bytes: None,
            error: Some("not found".into()),
        }],
        summary: BatchSummary {
            succeeded: 0,
            failed: 1,
            total_bytes: 0,
        },
    };
    let json = serde_json::to_value(&result).unwrap();
    let bug = &json["bug_results"][0];
    assert!(bug.get("files").is_none(), "files should skip when empty");
    assert!(bug.get("error").is_none(), "error should skip when None");
    let att = &json["attachment_results"][0];
    assert!(att.get("path").is_none());
    assert!(att.get("bytes").is_none());
    assert!(att.get("bug_id").is_none());
    assert_eq!(att["error"], "not found");
}

#[test]
fn target_status_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&TargetStatus::Ok).unwrap(), "\"ok\"");
    assert_eq!(
        serde_json::to_string(&TargetStatus::Error).unwrap(),
        "\"error\"",
    );
}

fn sample_batch_result() -> AttachmentBatchResult {
    AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![BugDownloadResult {
            bug_id: 12345,
            status: TargetStatus::Ok,
            files: vec![
                DownloadedFile {
                    attachment_id: 9876,
                    path: "./attachments/12345/9876.patch.diff".into(),
                    bytes: 4096,
                },
                DownloadedFile {
                    attachment_id: 9877,
                    path: "./attachments/12345/9877.trace.log".into(),
                    bytes: 2048,
                },
            ],
            error: None,
        }],
        attachment_results: vec![],
        summary: BatchSummary {
            succeeded: 2,
            failed: 0,
            total_bytes: 6144,
        },
    }
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachment_batch_table_includes_summary() {
    let _lock = crate::ENV_LOCK.lock().await;
    let result = sample_batch_result();
    let ((), out) = crate::test_helpers::capture_stdout(async {
        print_attachment_batch(&result, OutputFormat::Table);
    })
    .await;
    assert!(out.contains("Bug #12345"), "missing bug header: {out}");
    assert!(
        out.contains("./attachments/12345/9876.patch.diff"),
        "missing file path: {out}",
    );
    assert!(out.contains("2 succeeded"), "missing summary line: {out}",);
    assert!(out.contains("6144"), "missing total_bytes: {out}");
}

#[cfg(unix)]
#[tokio::test]
async fn print_attachment_batch_json_emits_typed_payload() {
    let _lock = crate::ENV_LOCK.lock().await;
    let result = sample_batch_result();
    let ((), out) = crate::test_helpers::capture_stdout(async {
        print_attachment_batch(&result, OutputFormat::Json);
    })
    .await;
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["bug_results"][0]["files"][0]["attachment_id"], 9876);
}
