#![expect(clippy::unwrap_used)]

use base64::Engine;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::AttachmentAction;
use crate::test_helpers::{make_attachment, setup_empty_config_env, setup_test_env};
use crate::types::OutputFormat;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn bug_attachments_response(bug_id: u64, atts: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "bugs": { bug_id.to_string(): atts },
    })
}

fn one_att(id: u64, bug_id: u64, file_name: &str, body: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "bug_id": bug_id,
        "file_name": file_name,
        "summary": file_name,
        "content_type": "text/plain",
        "creator": "dev@test.com",
        "creation_time": "2025-01-01T00:00:00Z",
        "last_change_time": "2025-01-01T00:00:00Z",
        "is_obsolete": false,
        "is_patch": false,
        "is_private": false,
        "size": body.len(),
        "data": b64(body),
    })
}

fn xmlrpc_one_att(id: u64, bug_id: u64, file_name: &str, body: &[u8]) -> String {
    format!(
        "<value><struct>\
            <member><name>id</name><value><int>{id}</int></value></member>\
            <member><name>bug_id</name><value><int>{bug_id}</int></value></member>\
            <member><name>file_name</name><value><string>{file_name}</string></value></member>\
            <member><name>summary</name><value><string>{file_name}</string></value></member>\
            <member><name>content_type</name><value><string>text/plain</string></value></member>\
            <member><name>creator</name><value><string>dev@test.com</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20250101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20250101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_patch</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>size</name><value><int>{}</int></value></member>\
            <member><name>data</name><value><base64>{}</base64></value></member>\
        </struct></value>",
        body.len(),
        b64(body),
    )
}

fn xmlrpc_bug_attachments_response(bug_id: u64, entries: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>{bug_id}</name><value><array><data>{entries}</data></array></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

#[tokio::test]
async fn attachment_download_api_error_propagates() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/404"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 100,
            "message": "Attachment 404 does not exist."
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Download {
        ids: vec![404],
        bug_ids: vec![],
        out: None,
        out_dir: "./attachments".into(),
    };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn attachment_download_validation_rejects_no_ids_no_bugs() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![],
        out: None,
        out_dir: "./attachments".into(),
    };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected InputValidation");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("specify at least one attachment ID"),
        "unexpected error: {msg}",
    );
}

#[tokio::test]
async fn attachment_download_validation_rejects_out_with_multiple_ids() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let action = AttachmentAction::Download {
        ids: vec![100, 200],
        bug_ids: vec![],
        out: Some("file.bin".into()),
        out_dir: "./attachments".into(),
    };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected InputValidation");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("--out requires exactly one attachment ID"),
        "unexpected error: {msg}",
    );
}

#[tokio::test]
async fn write_one_attachment_writes_inline_data_with_att_id_prefix() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();

    let mut att = make_attachment(
        9876,
        12345,
        "patch.diff",
        "Fix patch",
        Some(b64(b"Hello world")),
    );
    att.content_type = "text/x-diff".into();
    att.size = 11;
    att.is_patch = true;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let expected_path = tmp.path().join("12345").join("9876.patch.diff");
    assert!(expected_path.exists(), "{expected_path:?} not found");
    assert_eq!(std::fs::read(&expected_path).unwrap(), b"Hello world");
    assert_eq!(file.attachment_id, 9876);
    assert_eq!(file.bytes, 11);
}

#[tokio::test]
async fn write_one_attachment_falls_back_when_data_missing() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": {
                    "id": 9876,
                    "bug_id": 12345,
                    "file_name": "patch.diff",
                    "summary": "Fix patch",
                    "content_type": "text/plain",
                    "size": 11,
                    "data": b64(b"Hello world")
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();

    let mut att = make_attachment(9876, 12345, "patch.diff", "Fix patch", None);
    att.size = 11;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    assert_eq!(file.bytes, 11);
    let expected_path = tmp.path().join("12345").join("9876.patch.diff");
    assert!(expected_path.exists());
}

#[tokio::test]
async fn write_one_attachment_overwrites_existing_file() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();

    let dir = tmp.path().join("12345");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("9876.patch.diff"), b"OLD CONTENT").unwrap();

    let mut att = make_attachment(9876, 12345, "patch.diff", "v2", Some(b64(b"NEW CONTENT")));
    att.size = 11;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let written = std::fs::read(dir.join("9876.patch.diff")).unwrap();
    assert_eq!(written, b"NEW CONTENT");
}

#[tokio::test]
async fn write_one_attachment_create_dir_error_names_destination() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();
    let out_file = tmp.path().join("not-a-directory");
    std::fs::write(&out_file, b"file").unwrap();
    let att = make_attachment(
        9876,
        12345,
        "patch.diff",
        "Fix patch",
        Some(b64(b"content")),
    );

    let err = super::write_one_attachment(&client, &att, &out_file.to_string_lossy())
        .await
        .unwrap_err()
        .to_string();
    let expected_dir = out_file.join("12345");
    let expected_dir = expected_dir.to_string_lossy();

    assert!(
        err.contains("create attachment download directory") && err.contains(expected_dir.as_ref()),
        "error should name download directory, got: {err}"
    );
}

#[tokio::test]
async fn attachment_download_batch_per_bug_writes_per_bug_subdir() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([
                    one_att(9876, 12345, "patch.diff", b"alpha"),
                    one_att(9877, 12345, "trace.log", b"bravo charlie"),
                ]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io_a4 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a4.writers(),
    )
    .await;

    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["summary"]["failed"], 0);
    assert_eq!(parsed["summary"]["total_bytes"], 5 + 13);
    assert_eq!(parsed["bug_results"][0]["bug_id"], 12345);
    assert_eq!(parsed["bug_results"][0]["status"], "ok");

    assert!(tmp.path().join("12345").join("9876.patch.diff").exists());
    assert!(tmp.path().join("12345").join("9877.trace.log").exists());
    let p1 = std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap();
    assert_eq!(p1, b"alpha");
}

#[tokio::test]
async fn attachment_download_batch_hybrid_uses_xmlrpc_inline_data_without_fallback() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let entries = format!(
        "{}{}",
        xmlrpc_one_att(9876, 12345, "patch.diff", b"alpha"),
        xmlrpc_one_att(9877, 12345, "trace.log", b"bravo"),
    );

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_bug_attachments_response(12345, &entries)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9877"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(
            None,
            OutputFormat::Json,
            Some(crate::types::ApiMode::Hybrid),
        ),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "expected ok, got {result:?}");
    assert_eq!(
        std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap(),
        b"alpha",
    );
    assert_eq!(
        std::fs::read(tmp.path().join("12345").join("9877.trace.log")).unwrap(),
        b"bravo",
    );
}

#[tokio::test]
async fn attachment_download_batch_collision_filenames_resolved_by_att_id_prefix() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([
                    one_att(9876, 12345, "trace.log", b"first"),
                    one_att(9877, 12345, "trace.log", b"second"),
                ]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;

    let _ = __io.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let p1 = tmp.path().join("12345").join("9876.trace.log");
    let p2 = tmp.path().join("12345").join("9877.trace.log");
    assert_eq!(std::fs::read(&p1).unwrap(), b"first");
    assert_eq!(std::fs::read(&p2).unwrap(), b"second");
}

#[tokio::test]
async fn attachment_download_batch_mixed_bug_and_positional_ids() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([one_att(9876, 12345, "patch.diff", b"from bug")]),
            )),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/4242"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "4242": one_att(4242, 67890, "extra.bin", b"from positional"),
            }
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![4242],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io_a5 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a5.writers(),
    )
    .await;

    let output = __io_a5.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["bug_results"][0]["bug_id"], 12345);
    assert_eq!(parsed["attachment_results"][0]["attachment_id"], 4242);
    assert_eq!(parsed["attachment_results"][0]["bug_id"], 67890);

    assert!(tmp.path().join("12345").join("9876.patch.diff").exists());
    assert!(tmp.path().join("67890").join("4242.extra.bin").exists());
}

#[tokio::test]
async fn attachment_download_batch_empty_bug_zero_files_success() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bug_attachments_response(12345, &serde_json::json!([]))),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io_a6 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a6.writers(),
    )
    .await;

    let output = __io_a6.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["bug_results"][0]["status"], "ok");
    assert_eq!(parsed["summary"]["succeeded"], 0);
    assert_eq!(parsed["summary"]["failed"], 0);
}

#[tokio::test]
async fn attachment_download_batch_legacy_single_id_unchanged() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "patch.diff", b"legacy"),
            }
        })))
        .mount(&mock)
        .await;

    let out_path = tmp.path().join("downloaded.bin");
    let action = AttachmentAction::Download {
        ids: vec![9876],
        bug_ids: vec![],
        out: Some(out_path.to_string_lossy().into_owned()),
        out_dir: "./attachments".into(),
    };

    let mut __io_a7 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a7.writers(),
    )
    .await;

    let output = __io_a7.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    // Legacy path emits DownloadResult, not AttachmentBatchResult — the
    // JSON has `id` at the top level, not `summary`.
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 9876);
    assert_eq!(parsed["size"].as_u64().unwrap_or(0), 6);
    assert!(out_path.exists());
    assert_eq!(std::fs::read(&out_path).unwrap(), b"legacy");
}

#[tokio::test]
async fn attachment_download_single_out_dash_streams_bytes_without_result() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "trace.bin", b"raw\0bytes\n"),
            }
        })))
        .mount(&mock)
        .await;

    let dash_path = std::path::Path::new("-");
    assert!(
        !dash_path.exists(),
        "stdout-mode test requires no pre-existing file named '-' in the cwd",
    );

    let action = AttachmentAction::Download {
        ids: vec![9876],
        bug_ids: vec![],
        out: Some("-".into()),
        out_dir: "./attachments".into(),
    };
    let mut io = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let wrote_dash_file = dash_path.exists();
    if wrote_dash_file {
        std::fs::remove_file(dash_path).unwrap();
    }

    assert!(result.is_ok(), "expected stdout download, got {result:?}");
    assert_eq!(io.out, b"raw\0bytes\n");
    assert!(
        io.err.is_empty(),
        "stderr should stay empty: {:?}",
        io.err_str()
    );
    assert!(
        !wrote_dash_file,
        "--out - must stream to stdout, not create a file named '-'",
    );
}

#[tokio::test]
async fn attachment_download_batch_bug_not_found_partial_failure() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([one_att(9876, 12345, "patch.diff", b"ok")]),
            )),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/99999/attachment"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 101,
            "message": "Bug 99999 does not exist."
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345, 99999],
        out: None,
        out_dir,
    };

    let mut __io_a8 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a8.writers(),
    )
    .await;

    let output = __io_a8.out_str().to_string();
    assert!(result.is_err(), "expected BatchPartialFailure");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 11, "expected exit 11, got {exit}: {err}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 1);
    assert_eq!(parsed["summary"]["failed"], 1);
    let bugs = parsed["bug_results"].as_array().unwrap();
    assert_eq!(bugs[0]["bug_id"], 12345);
    assert_eq!(bugs[0]["status"], "ok");
    assert_eq!(bugs[1]["bug_id"], 99999);
    assert_eq!(bugs[1]["status"], "error");
}

#[tokio::test]
async fn attachment_download_batch_creates_out_dir_before_connect() {
    let (_lock, tmp) = setup_empty_config_env().await;
    let out_dir = tmp.path().join("downloaded");
    let action = AttachmentAction::Download {
        ids: vec![9876],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.to_string_lossy().into_owned(),
    };

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_err(),
        "missing config should still fail after preflight"
    );
    assert!(
        out_dir.is_dir(),
        "batch download should create out_dir before connecting"
    );
}

#[tokio::test]
async fn attachment_download_batch_all_targets_fail_still_exit_11() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/99999/attachment"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 101,
            "message": "Bug 99999 does not exist."
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![99999],
        out: None,
        out_dir,
    };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected BatchPartialFailure");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 11, "all-fail still uses BatchPartialFailure: {err}");
}

#[tokio::test]
async fn attachment_download_batch_obsolete_attachments_included() {
    let (_lock, mock, tmp) = setup_test_env().await;

    let mut obsolete = one_att(9876, 12345, "old.patch", b"obsolete content");
    obsolete["is_obsolete"] = serde_json::json!(true);
    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([obsolete]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    assert!(tmp.path().join("12345").join("9876.old.patch").exists());
}

#[tokio::test]
async fn attachment_download_batch_data_missing_falls_back_via_get() {
    let (_lock, mock, tmp) = setup_test_env().await;

    // Listing returns the attachment metadata WITHOUT data.
    let mut att = one_att(9876, 12345, "patch.diff", b"x");
    att["data"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bug_attachments_response(12345, &serde_json::json!([att]))),
        )
        .mount(&mock)
        .await;

    // Fallback fetch DOES return data.
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "patch.diff", b"fallback"),
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io3.writers(),
    )
    .await;

    let _ = __io3.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    let written = std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap();
    assert_eq!(written, b"fallback");
}

#[cfg(unix)]
#[tokio::test]
async fn attachment_download_batch_top_level_out_dir_unwritable_fails_fast() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // /dev/null/attachments — create_dir_all on a path under /dev/null
    // (which is a character device, not a directory) → ENOTDIR.
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: "/dev/null/attachments".into(),
    };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected Io");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 6, "expected Io exit 6 (fail-fast), got {exit}: {err}");
}

#[tokio::test]
async fn write_one_attachment_invalid_base64_returns_data_integrity() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();

    let mut att = make_attachment(
        9876,
        12345,
        "patch.diff",
        "broken",
        Some("not valid base64 !!".into()),
    );
    att.size = 0;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let result = super::write_one_attachment(&client, &att, &out_dir).await;
    assert!(result.is_err(), "expected DataIntegrity for invalid base64");
    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::DataIntegrity(_)),
        "expected DataIntegrity, got {err}",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("decode attachment #9876"),
        "expected decode error message including att-id, got: {msg}",
    );
}

// ── filename sanitization (path traversal) ──────────────────────────

#[test]
fn safe_basename_strips_directory_components() {
    assert_eq!(super::safe_basename("normal.txt").unwrap(), "normal.txt");
    assert_eq!(super::safe_basename("../../etc/passwd").unwrap(), "passwd");
    assert_eq!(super::safe_basename("/etc/passwd").unwrap(), "passwd");
    assert_eq!(super::safe_basename("a/b/c.diff").unwrap(), "c.diff");
}

#[test]
fn safe_basename_rejects_names_without_a_basename() {
    assert!(super::safe_basename("").is_err());
    assert!(super::safe_basename("..").is_err());
    assert!(super::safe_basename(".").is_err());
    assert!(super::safe_basename("foo/..").is_err());
}

#[test]
fn single_download_dest_honors_explicit_out_verbatim() {
    let dest = super::single_download_dest(Some("/tmp/user/chosen.bin"), "server.txt").unwrap();
    assert_eq!(dest, std::path::Path::new("/tmp/user/chosen.bin"));
}

#[test]
fn single_download_dest_sanitizes_server_filename_when_no_out() {
    let dest = super::single_download_dest(None, "../../escape.txt").unwrap();
    assert_eq!(dest, std::path::Path::new("escape.txt"));
}

#[tokio::test]
async fn write_one_attachment_sanitizes_server_filename_with_separators() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = crate::commands::runtime::shared::connect_and_configure(
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
    )
    .await
    .unwrap();

    let att = make_attachment(7, 42, "sub/dir/escape.txt", "evil", Some(b64(b"data")));
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let expected = tmp.path().join("42").join("7.escape.txt");
    assert_eq!(std::path::Path::new(&file.path), expected);
    assert!(expected.exists(), "{expected:?} not found");
}
