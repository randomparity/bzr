#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, TemplateAction};
use crate::error::BzrError;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

fn create_action() -> BugAction {
    BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: Some("New bug".into()),
        version: Some("unspecified".into()),
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    }
}

#[tokio::test]
async fn bug_create_sends_post() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 99})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &create_action(),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn bug_create_missing_product_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: None,
        component: Some("General".into()),
        summary: Some("Needs product".into()),
        version: None,
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(msg) if msg.contains("--product")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_missing_component_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: None,
        summary: Some("Needs component".into()),
        version: None,
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(msg) if msg.contains("--component")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_with_unknown_template_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: Some("does-not-exist".into()),
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: Some("Bad template".into()),
        version: None,
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::Config(msg) if msg.contains("does-not-exist")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_with_template_fills_missing_fields() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Pre-populate a template with product/component/version so the
    // bug create command can resolve them from the template.
    let save = TemplateAction::Save {
        name: "tpl".into(),
        product: Some("TplProduct".into()),
        component: Some("TplComponent".into()),
        version: Some("9.9".into()),
        priority: Some("P2".into()),
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("from template".into()),
    };
    let (result, _) = capture_stdout(crate::commands::template::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "template save failed: {result:?}");

    // The mock should see the template's product/component/version
    // forwarded into the POST body.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("TplProduct"))
        .and(body_string_contains("TplComponent"))
        .and(body_string_contains("9.9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Create {
        template: Some("tpl".into()),
        product: None,
        component: None,
        summary: Some("From template".into()),
        version: None,
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(
        result.is_ok(),
        "bug create with template failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["id"], 7);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn bug_create_reads_description_from_file() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let dir = std::env::temp_dir();
    let desc_path = dir.join(format!("bzr-create-desc-{}.txt", std::process::id()));
    std::fs::write(&desc_path, "description from file\n").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("description from file"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 11})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: Some("Bug from file".into()),
        version: None,
        description: None,
        description_file: Some(desc_path.clone()),
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "got {result:?}");
    let _ = std::fs::remove_file(&desc_path);
}

#[tokio::test]
async fn bug_create_description_file_missing_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: Some("Bug".into()),
        version: None,
        description: None,
        description_file: Some(std::path::PathBuf::from("/nonexistent-bzr-test-path-xyz")),
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(m) if m.contains("description-file")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_description_file_non_utf8_returns_input_validation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let dir = std::env::temp_dir();
    let bad_path = dir.join(format!("bzr-create-bad-utf8-{}.bin", std::process::id()));
    std::fs::write(&bad_path, [0xff_u8, 0xfe_u8, 0xfd_u8]).unwrap();

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: Some("Bug".into()),
        version: None,
        description: None,
        description_file: Some(bad_path.clone()),
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    let _ = std::fs::remove_file(&bad_path);
    assert!(
        matches!(&err, BzrError::InputValidation(m) if m.contains("description-file")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_missing_summary_without_editor_flow_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: None,
        version: None,
        description: Some("body".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(m) if m.contains("--summary")),
        "got {err:?}"
    );
}

#[test]
fn parse_editor_buffer_strips_sentinel_and_extracts_summary() {
    let buf = "\
My bug summary

This is the description.

# ------------------------ >8 ------------------------
# Do not modify or remove the line above.
# Product: Foo
";
    let (summary, description) = super::parse_editor_buffer(buf).unwrap();
    assert_eq!(summary, "My bug summary");
    assert_eq!(description, "This is the description.");
}

#[test]
fn parse_editor_buffer_handles_multi_line_summary_block() {
    let buf = "\
Summary line
overflow line

Description here

# ------------------------ >8 ------------------------
# trailer
";
    let (summary, description) = super::parse_editor_buffer(buf).unwrap();
    assert_eq!(summary, "Summary line");
    assert_eq!(description, "overflow line\n\nDescription here");
}

#[test]
fn parse_editor_buffer_skips_leading_blank_lines() {
    let buf =
        "\n\nReal summary\n\nReal body\n\n# ------------------------ >8 ------------------------\n";
    let (summary, description) = super::parse_editor_buffer(buf).unwrap();
    assert_eq!(summary, "Real summary");
    assert_eq!(description, "Real body");
}

#[test]
fn parse_editor_buffer_empty_above_sentinel_errors() {
    let buf = "\
# ------------------------ >8 ------------------------
# Product: Foo
# Component: Bar
";
    let err = super::parse_editor_buffer(buf).unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(m) if m.contains("empty buffer")),
        "got {err:?}"
    );
}

#[test]
fn parse_editor_buffer_no_sentinel_uses_full_buffer() {
    let buf = "Summary\n\nDescription\n";
    let (summary, description) = super::parse_editor_buffer(buf).unwrap();
    assert_eq!(summary, "Summary");
    assert_eq!(description, "Description");
}

#[test]
fn parse_editor_buffer_only_summary_no_description() {
    let buf = "\
Just a summary

# ------------------------ >8 ------------------------
";
    let (summary, description) = super::parse_editor_buffer(buf).unwrap();
    assert_eq!(summary, "Just a summary");
    assert_eq!(description, "");
}

#[test]
fn build_editor_template_includes_summary_and_field_reminder() {
    use crate::types::CreateBugParams;
    let params = CreateBugParams {
        product: "Foo".into(),
        component: "Bar".into(),
        summary: String::new(),
        version: "1.0".into(),
        description: None,
        priority: None,
        severity: Some("High".into()),
        assigned_to: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        keywords: vec![],
    };
    let buf = super::build_editor_template(Some("Pre-filled summary"), None, &params);
    assert!(buf.starts_with("Pre-filled summary\n"));
    assert!(buf.contains("# ------------------------ >8 ------------------------"));
    assert!(buf.contains("# Product:    Foo"));
    assert!(buf.contains("# Component:  Bar"));
    assert!(buf.contains("# Severity:   High"));
    assert!(buf.contains("# Priority:   <unset>"));
}

#[test]
fn build_editor_template_includes_template_description_body() {
    use crate::types::CreateBugParams;
    let params = CreateBugParams {
        product: "Foo".into(),
        component: "Bar".into(),
        summary: String::new(),
        version: "1.0".into(),
        description: None,
        priority: None,
        severity: None,
        assigned_to: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        keywords: vec![],
    };
    let buf = super::build_editor_template(None, Some("## Steps\n\n## Expected"), &params);
    assert!(buf.contains("## Steps"));
    assert!(buf.contains("## Expected"));
}

#[tokio::test]
async fn bug_create_editor_flow_uses_editor_buffer_for_summary_and_description() {
    use std::io::IsTerminal;
    use std::os::unix::fs::PermissionsExt;

    let (_lock, mock, _tmp) = setup_test_env().await;

    let dir = std::env::temp_dir();
    let script = dir.join(format!("bzr-bc-editor-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'Editor summary\\n\\nEditor description\\n' > \"$1\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev = std::env::var("EDITOR").ok();
    // SAFETY: setup_test_env holds bzr::ENV_LOCK for the duration of
    // this test, serializing env access across all tests using it.
    unsafe { std::env::set_var("EDITOR", &script) };

    // The expect range covers both branches: when stdin is a TTY,
    // the editor flow runs and POST is made (1 hit); when cargo
    // test pipes stdin, the stdin branch hits InputValidation
    // before any HTTP call (0 hits).
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("Editor summary"))
        .and(body_string_contains("Editor description"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 33})))
        .expect(0..=1)
        .mount(&mock)
        .await;

    let action = BugAction::Create {
        template: None,
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        summary: None,
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
    };
    let (result, _output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;

    // SAFETY: setup_test_env holds bzr::ENV_LOCK for the duration of
    // this test, serializing env access across all tests using it.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let _ = std::fs::remove_file(&script);

    if std::io::stdin().is_terminal() {
        assert!(result.is_ok(), "editor flow should succeed: {result:?}");
    } else {
        // Stdin branch: empty piped stdin + summary=None
        // → InputValidation about empty stdin.
        let err = result.unwrap_err();
        assert!(matches!(&err, BzrError::InputValidation(_)), "got {err:?}");
    }
}
