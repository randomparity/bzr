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

fn capture(format: OutputFormat, attachments: &[Attachment]) -> String {
    let mut buf = Vec::new();
    write_attachments(attachments, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_batch(format: OutputFormat, result: &AttachmentBatchResult) -> (String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_attachment_batch(result, format, &mut out, &mut err);
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn write_attachments_json_empty() {
    let attachments: Vec<Attachment> = vec![];
    let json = serde_json::to_string_pretty(&attachments).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_attachments_json_one_attachment() {
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
fn write_attachments_json_obsolete_and_private() {
    let mut att = make_attachment(11, "Old patch");
    att.is_obsolete = true;
    att.is_private = true;
    let json = serde_json::to_string(&att).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["is_obsolete"], true);
    assert_eq!(parsed["is_private"], true);
}

#[test]
fn write_attachments_table_empty_says_no_attachments() {
    let output = capture(OutputFormat::Table, &[]);
    assert!(output.contains("No attachments."));
}

#[test]
fn write_attachments_json_empty_renders_empty_array() {
    let output = capture(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_attachments_table_renders_all_fields() {
    let mut att = make_attachment(7, "Helpful patch");
    att.is_obsolete = true;
    att.is_private = true;
    let output = capture(OutputFormat::Table, &[att]);
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

#[test]
fn write_attachments_table_missing_optional_fields_render_dash() {
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
    let output = capture(OutputFormat::Table, &attachments);
    assert!(output.contains("Unicode summary — é"));
    assert!(output.contains("patché.txt"));
    assert!(
        output.contains("Creator       -"),
        "expected dashed Creator field, got: {output}"
    );
    assert!(
        output.contains("Created       -"),
        "expected dashed Created field, got: {output}"
    );
    assert!(!output.contains("[OBSOLETE]"));
    assert!(!output.contains("[PRIVATE]"));
}

#[test]
fn write_attachments_table_renders_patch_tag() {
    let mut att = make_attachment(12, "Patch attachment");
    att.is_patch = true;
    let output = capture(OutputFormat::Table, &[att]);
    assert!(output.contains("Attachment"));
    assert!(output.contains("#12"));
    assert!(output.contains("Patch attachment"));
    assert!(output.contains("[PATCH]"));
    assert!(!output.contains("[OBSOLETE]"));
    assert!(!output.contains("[PRIVATE]"));
}

#[test]
fn write_attachments_table_patch_tag_precedes_obsolete_and_private() {
    let mut att = make_attachment(13, "All flags");
    att.is_patch = true;
    att.is_obsolete = true;
    att.is_private = true;
    let output = capture(OutputFormat::Table, &[att]);
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

#[test]
fn write_attachments_json_one_via_write() {
    let output = capture(OutputFormat::Json, &[make_attachment(99, "Json patch")]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 99);
    assert_eq!(parsed[0]["summary"], "Json patch");
    assert_eq!(parsed[0]["file_name"], "file_99.patch");
}

#[test]
fn write_attachments_does_not_emit_ansi_when_writing_to_buffer() {
    let output = capture(OutputFormat::Table, &[make_attachment(1, "p")]);
    assert!(
        !output.contains('\x1b'),
        "expected no ANSI escapes when writing to Vec<u8>: {output:?}",
    );
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

#[test]
fn write_attachment_batch_table_includes_summary() {
    let result = sample_batch_result();
    let (out, _err) = capture_batch(OutputFormat::Table, &result);
    assert!(out.contains("Bug #12345"), "missing bug header: {out}");
    assert!(
        out.contains("./attachments/12345/9876.patch.diff"),
        "missing file path: {out}",
    );
    assert!(out.contains("2 succeeded"), "missing summary line: {out}");
    assert!(out.contains("6144"), "missing total_bytes: {out}");
}

#[test]
fn write_attachment_batch_json_emits_typed_payload() {
    let result = sample_batch_result();
    let (out, _err) = capture_batch(OutputFormat::Json, &result);
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["bug_results"][0]["files"][0]["attachment_id"], 9876);
}

#[test]
fn write_attachment_batch_table_bug_error_writes_to_err() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![BugDownloadResult {
            bug_id: 99999,
            status: TargetStatus::Error,
            files: vec![],
            error: Some("bug not found".into()),
        }],
        attachment_results: vec![],
        summary: BatchSummary {
            succeeded: 0,
            failed: 1,
            total_bytes: 0,
        },
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_attachment_batch_table(&result, &mut out, &mut err);
    let err_str = String::from_utf8(err).unwrap();
    let out_str = String::from_utf8(out).unwrap();
    assert!(
        err_str.contains("Bug #99999: bug not found"),
        "stderr should carry bug-error: {err_str}",
    );
    assert!(out_str.contains("0 succeeded, 1 failed"));
}

#[test]
fn write_attachment_batch_table_bug_error_with_partial_files() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![BugDownloadResult {
            bug_id: 12345,
            status: TargetStatus::Error,
            files: vec![DownloadedFile {
                attachment_id: 9876,
                path: "./attachments/12345/9876.a.txt".into(),
                bytes: 5,
            }],
            error: Some("write failed for #9877".into()),
        }],
        attachment_results: vec![],
        summary: BatchSummary {
            succeeded: 1,
            failed: 1,
            total_bytes: 5,
        },
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_attachment_batch_table(&result, &mut out, &mut err);
    let out_str = String::from_utf8(out).unwrap();
    let err_str = String::from_utf8(err).unwrap();
    assert!(
        err_str.contains("Bug #12345: write failed for #9877"),
        "stderr should carry bug-error: {err_str}",
    );
    assert!(
        out_str.contains("./attachments/12345/9876.a.txt") && out_str.contains("[partial]"),
        "stdout should list partial file with [partial] tag: {out_str}",
    );
}

#[test]
fn write_attachment_batch_table_attachment_error_writes_to_err() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![],
        attachment_results: vec![AttachmentDownloadResult {
            attachment_id: 4242,
            status: TargetStatus::Error,
            bug_id: None,
            path: None,
            bytes: None,
            error: Some("attachment 4242 not found".into()),
        }],
        summary: BatchSummary {
            succeeded: 0,
            failed: 1,
            total_bytes: 0,
        },
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_attachment_batch_table(&result, &mut out, &mut err);
    let err_str = String::from_utf8(err).unwrap();
    assert!(
        err_str.contains("Attachment #4242: attachment 4242 not found"),
        "stderr should carry attachment-error: {err_str}",
    );
}

#[test]
fn write_attachment_batch_table_attachment_ok_writes_path_and_bytes() {
    let result = AttachmentBatchResult {
        out_dir: "./attachments".into(),
        bug_results: vec![],
        attachment_results: vec![AttachmentDownloadResult {
            attachment_id: 4242,
            status: TargetStatus::Ok,
            bug_id: Some(67890),
            path: Some("./attachments/67890/4242.extra.bin".into()),
            bytes: Some(256),
            error: None,
        }],
        summary: BatchSummary {
            succeeded: 1,
            failed: 0,
            total_bytes: 256,
        },
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_attachment_batch_table(&result, &mut out, &mut err);
    let out_str = String::from_utf8(out).unwrap();
    assert!(
        out_str.contains("Attachment") && out_str.contains("#4242"),
        "stdout should include attachment header: {out_str}",
    );
    assert!(out_str.contains("./attachments/67890/4242.extra.bin"));
    assert!(out_str.contains("256"));
}

#[test]
fn batch_summary_from_results_aggregates_correctly() {
    let bug_results = vec![
        BugDownloadResult {
            bug_id: 1,
            status: TargetStatus::Ok,
            files: vec![
                DownloadedFile {
                    attachment_id: 10,
                    path: "p1".into(),
                    bytes: 100,
                },
                DownloadedFile {
                    attachment_id: 11,
                    path: "p2".into(),
                    bytes: 200,
                },
            ],
            error: None,
        },
        BugDownloadResult {
            bug_id: 2,
            status: TargetStatus::Error,
            files: vec![],
            error: Some("missing".into()),
        },
    ];
    let attachment_results = vec![
        AttachmentDownloadResult {
            attachment_id: 30,
            status: TargetStatus::Ok,
            bug_id: Some(3),
            path: Some("p3".into()),
            bytes: Some(50),
            error: None,
        },
        AttachmentDownloadResult {
            attachment_id: 31,
            status: TargetStatus::Error,
            bug_id: None,
            path: None,
            bytes: None,
            error: Some("missing".into()),
        },
    ];
    let summary = BatchSummary::from_results(&bug_results, &attachment_results);
    assert_eq!(summary.succeeded, 3);
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.total_bytes, 350);
}
