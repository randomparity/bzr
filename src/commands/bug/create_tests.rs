#![expect(clippy::unwrap_used)]

use std::io::Write;

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, TemplateAction};
use crate::error::BzrError;
use crate::test_helpers::setup_test_env;
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
        create_fields: crate::cli::CreateFieldArgs::default(),
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

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &create_action(),
        None,
        OutputFormat::Json,
        None,
        &mut __io.writers(),
    )
    .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn bug_create_sends_parity_fields_in_body() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Every parity field must appear in the POST body. A request missing any
    // of these matchers won't match the mock, so the call would 404 and fail.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"alias\":\"a-1\""))
        .and(body_string_contains(
            "\"url\":\"https://example.com/repro\"",
        ))
        .and(body_string_contains("\"whiteboard\":\"needs-triage\""))
        .and(body_string_contains("\"target_milestone\":\"M1\""))
        .and(body_string_contains("\"deadline\":\"2026-12-31\""))
        .and(body_string_contains("\"cc\":[\"cc@example.com\"]"))
        .and(body_string_contains("\"keywords\":[\"regression\"]"))
        .and(body_string_contains("\"groups\":[\"security\"]"))
        .and(body_string_contains(
            "\"flags\":[{\"name\":\"review\",\"status\":\"+\"}]",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 123})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Create {
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
        create_fields: crate::cli::CreateFieldArgs {
            alias: Some("a-1".into()),
            url: Some("https://example.com/repro".into()),
            whiteboard: Some("needs-triage".into()),
            target_milestone: Some("M1".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cc@example.com".into()],
            keywords: vec!["regression".into()],
            groups: vec!["security".into()],
            flag: vec!["review+".into()],
        },
    };

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    assert!(
        result.is_ok(),
        "create with parity fields failed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(parsed["id"], 123);
}

#[tokio::test]
async fn bug_create_rejects_malformed_deadline() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create {
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
        create_fields: crate::cli::CreateFieldArgs {
            deadline: Some("not-a-date".into()),
            ..Default::default()
        },
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(msg) if msg.contains("--deadline")),
        "got {err:?}"
    );
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;
    let _output = __io2.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;
    let _output = __io3.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;
    let _output = __io4.out_str().to_string();
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
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;
    let _ = __io5.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io6 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io6.writers(),
    )
    .await;
    let output = __io6.out_str().to_string();
    assert!(
        result.is_ok(),
        "bug create with template failed: {result:?}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io7.writers(),
    )
    .await;
    let _output = __io7.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io8 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io8.writers(),
    )
    .await;
    let _output = __io8.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io9 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io9.writers(),
    )
    .await;
    let _output = __io9.out_str().to_string();
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io10 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io10.writers(),
    )
    .await;
    let _output = __io10.out_str().to_string();
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
        alias: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        groups: vec![],
        flags: vec![],
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
        alias: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        groups: vec![],
        flags: vec![],
    };
    let buf = super::build_editor_template(None, Some("## Steps\n\n## Expected"), &params);
    assert!(buf.contains("## Steps"));
    assert!(buf.contains("## Expected"));
}

/// Write a fake `$EDITOR` script that emits a deterministic
/// summary+description payload. Returns the script path so the
/// caller can clean it up after the test.
fn install_fake_editor() -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir();
    let script = dir.join(format!("bzr-bc-editor-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'Editor summary\\n\\nEditor description\\n' > \"$1\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn editor_action_no_summary_no_description() -> BugAction {
    BugAction::Create {
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    }
}

#[tokio::test]
async fn bug_create_editor_flow_resolves_via_editor_when_stdin_is_tty() {
    use std::io::IsTerminal;

    if !std::io::stdin().is_terminal() {
        let _ = writeln!(
            std::io::stderr(),
            "Skipping: editor flow requires TTY stdin (cargo test runs non-TTY)."
        );
        return;
    }

    let (_lock, mock, _tmp) = setup_test_env().await;

    let script = install_fake_editor();
    let prev = std::env::var("EDITOR").ok();
    // SAFETY: setup_test_env holds bzr::ENV_LOCK for the duration of
    // this test, serializing env access across all tests using it.
    unsafe { std::env::set_var("EDITOR", &script) };

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("Editor summary"))
        .and(body_string_contains("Editor description"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 33})))
        .expect(1)
        .mount(&mock)
        .await;

    let mut __io11 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &editor_action_no_summary_no_description(),
        None,
        OutputFormat::Json,
        None,
        &mut __io11.writers(),
    )
    .await;

    let _output = __io11.out_str().to_string();

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

    assert!(result.is_ok(), "editor flow should succeed: {result:?}");
}

/// Deterministic CI counterpart: under cargo test, stdin is piped
/// (not a TTY), so the editor branch must NOT fire even with an
/// `$EDITOR` set. The empty piped stdin should hit `InputValidation`
/// before any HTTP call.
#[tokio::test]
async fn bug_create_editor_branch_unreachable_when_stdin_piped() {
    use std::io::IsTerminal;

    if std::io::stdin().is_terminal() {
        let _ = writeln!(
            std::io::stderr(),
            "Skipping: this test asserts the non-editor path under piped stdin."
        );
        return;
    }

    let (_lock, mock, _tmp) = setup_test_env().await;

    let script = install_fake_editor();
    let prev = std::env::var("EDITOR").ok();
    // SAFETY: setup_test_env holds bzr::ENV_LOCK for the duration of
    // this test, serializing env access across all tests using it.
    unsafe { std::env::set_var("EDITOR", &script) };

    // No HTTP call expected — empty piped stdin must short-circuit
    // before the editor branch and before any client request.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 0})))
        .expect(0)
        .mount(&mock)
        .await;

    let mut __io12 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &editor_action_no_summary_no_description(),
        None,
        OutputFormat::Json,
        None,
        &mut __io12.writers(),
    )
    .await;

    let _output = __io12.out_str().to_string();

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

    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(m) if m.contains("piped stdin")),
        "expected InputValidation about empty piped stdin, got {err:?}"
    );
}

#[tokio::test]
async fn bug_create_template_description_does_not_fall_back_outside_editor_flow() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Pre-populate a template that has a description body.
    let save = TemplateAction::Save {
        name: "tpl-with-desc".into(),
        product: Some("TestProduct".into()),
        component: Some("General".into()),
        version: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("template body".into()),
    };
    let mut __io13 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
        &mut __io13.writers(),
    )
    .await;
    let _ = __io13.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    // Invoke bug create with the template, no other description source,
    // under cargo's non-TTY stdin: the template description must NOT
    // be used as a fallback. The empty-stdin branch fires first and
    // returns InputValidation.
    let action = BugAction::Create {
        template: Some("tpl-with-desc".into()),
        product: None,
        component: None,
        summary: Some("Bug from template".into()),
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
        create_fields: crate::cli::CreateFieldArgs::default(),
    };
    let mut __io14 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io14.writers(),
    )
    .await;
    let _output = __io14.out_str().to_string();
    let err = result.unwrap_err();
    assert!(
        matches!(&err, BzrError::InputValidation(_)),
        "expected InputValidation (template body should not auto-fill outside the editor flow), got {err:?}"
    );
}

#[test]
fn resolve_description_conflict_errors() {
    let err =
        super::resolve_description(Some("x"), Some(std::path::Path::new("/tmp/x"))).unwrap_err();
    match err {
        BzrError::InputValidation(msg) => {
            assert!(msg.contains("--description"), "names inline flag: {msg}");
            assert!(msg.contains("--description-file"), "names file flag: {msg}");
        }
        other => panic!("expected InputValidation, got {other:?}"),
    }
}
