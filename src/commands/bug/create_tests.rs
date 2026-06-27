#![expect(clippy::unwrap_used, clippy::panic)]

use std::io::Write;

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, TemplateAction, TemplateFields};
use crate::error::BzrError;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn create_action() -> BugAction {
    BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    })
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn bug_create_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // A create POST must never fire under --dry-run. The connect-time TLS
    // probe is a HEAD, so it won't match this mock.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .expect(0)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &create_action(),
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["resource"], "bug");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["product"], "TestProduct");
    assert_eq!(parsed["changes"]["component"], "General");
    assert_eq!(parsed["changes"]["summary"], "New bug");
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
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
    });

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "create with parity fields failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed["id"], 123);
}

#[tokio::test]
async fn bug_create_rejects_malformed_deadline() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs {
            deadline: Some("not-a-date".into()),
            ..Default::default()
        },
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        fields: TemplateFields {
            product: Some("TplProduct".into()),
            component: Some("TplComponent".into()),
            version: Some("9.9".into()),
            priority: Some("P2".into()),
            description: Some("from template".into()),
            ..Default::default()
        },
    };
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &save,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io6 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io6.writers(),
    )
    .await;
    let output = __io6.out_str().to_string();
    assert!(
        result.is_ok(),
        "bug create with template failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 7);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn bug_create_template_applies_create_metadata_defaults() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save = TemplateAction::Save {
        name: "routing".into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            url: Some("https://example.com/repro".into()),
            whiteboard: Some("needs-triage".into()),
            target_milestone: Some("M1".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cc@example.com".into()],
            keywords: vec!["regression".into()],
            groups: vec!["security".into()],
            flag: vec!["review?(qa@example.com)".into()],
            ..Default::default()
        },
    };
    let mut save_io = crate::test_helpers::CapturedIo::new();
    crate::commands::template::execute(
        &save,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await
    .unwrap();
    let _ = save_io.out_str().to_string();

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
        template: Some("routing".into()),
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "template create dry-run failed: {result:?}");

    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    let changes = &parsed["changes"];
    assert_eq!(changes["url"], "https://example.com/repro");
    assert_eq!(changes["whiteboard"], "needs-triage");
    assert_eq!(changes["target_milestone"], "M1");
    assert_eq!(changes["deadline"], "2026-12-31");
    assert_eq!(changes["cc"], serde_json::json!(["cc@example.com"]));
    assert_eq!(changes["keywords"], serde_json::json!(["regression"]));
    assert_eq!(changes["groups"], serde_json::json!(["security"]));
    assert_eq!(changes["flags"][0]["name"], "review");
    assert_eq!(changes["flags"][0]["status"], "?");
    assert_eq!(changes["flags"][0]["requestee"], "qa@example.com");
}

#[tokio::test]
async fn bug_create_cli_create_metadata_overrides_template() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save = TemplateAction::Save {
        name: "routing".into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            url: Some("https://example.com/template".into()),
            whiteboard: Some("template-whiteboard".into()),
            target_milestone: Some("TemplateM".into()),
            deadline: Some("2026-01-01".into()),
            cc: vec!["template-cc@example.com".into()],
            keywords: vec!["template-keyword".into()],
            groups: vec!["template-group".into()],
            flag: vec!["review?".into()],
            ..Default::default()
        },
    };
    let mut save_io = crate::test_helpers::CapturedIo::new();
    crate::commands::template::execute(
        &save,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await
    .unwrap();
    let _ = save_io.out_str().to_string();

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
        template: Some("routing".into()),
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs {
            alias: None,
            url: Some("https://example.com/cli".into()),
            whiteboard: Some("cli-whiteboard".into()),
            target_milestone: Some("CliM".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cli-cc@example.com".into()],
            keywords: vec!["cli-keyword".into()],
            groups: vec!["cli-group".into()],
            flag: vec!["approval+".into()],
        },
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "template create dry-run failed: {result:?}");

    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    let changes = &parsed["changes"];
    assert_eq!(changes["url"], "https://example.com/cli");
    assert_eq!(changes["whiteboard"], "cli-whiteboard");
    assert_eq!(changes["target_milestone"], "CliM");
    assert_eq!(changes["deadline"], "2026-12-31");
    assert_eq!(changes["cc"], serde_json::json!(["cli-cc@example.com"]));
    assert_eq!(changes["keywords"], serde_json::json!(["cli-keyword"]));
    assert_eq!(changes["groups"], serde_json::json!(["cli-group"]));
    assert_eq!(changes["flags"][0]["name"], "approval");
    assert_eq!(changes["flags"][0]["status"], "+");
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io8 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io9 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io10 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    })
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            description: Some("template body".into()),
            ..Default::default()
        },
    };
    let mut __io13 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::template::execute(
        &save,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io13.writers(),
    )
    .await;
    let _ = __io13.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    // Invoke bug create with the template, no other description source,
    // under cargo's non-TTY stdin: the template description must NOT
    // be used as a fallback. The empty-stdin branch fires first and
    // returns InputValidation.
    let action = BugAction::Create(crate::cli::CreateArgs {
        from_json: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    });
    let mut __io14 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

// ── Structured input: bug create --from-json (#307) ──────────────────

/// Build a `from_json` create action: `from_json` set, every CLI field at its
/// default so the JSON is the sole field source unless a test overrides one.
fn from_json_action(path: &str) -> BugAction {
    BugAction::Create(crate::cli::CreateArgs {
        from_json: Some(path.to_string()),
        template: None,
        product: None,
        component: None,
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
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    })
}

/// Write `json` to a file under `tmp` and return its path, so tests exercise
/// the `--from-json <PATH>` branch without driving stdin.
fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn from_json_single_object_files_a_bug() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"product\":\"P\""))
        .and(body_string_contains("\"summary\":\"S\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"product":"P","component":"C","summary":"S"}"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "single object should file a bug: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 7);
}

#[tokio::test]
async fn from_json_array_batch_creates_one_per_element() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 11})))
        .expect(2)
        .mount(&mock)
        .await;

    let json = r#"[{"product":"P","component":"C","summary":"one"},
                   {"product":"P","component":"C","summary":"two"}]"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "array should batch-create: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["created"], serde_json::json!([11, 11]));
    assert_eq!(parsed["failed"], serde_json::json!([]));
}

#[tokio::test]
async fn from_json_array_partial_failure_exits_11() {
    let (_lock, mock, tmp) = setup_test_env().await;
    // First create succeeds (id 11); the component endpoint for the second is a
    // 400 with a Bugzilla error body, so it fails.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"summary\":\"ok\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 11})))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"summary\":\"bad\""))
        .respond_with(ResponseTemplate::new(400).set_body_json(
            serde_json::json!({"error": true, "code": 51, "message": "Invalid component"}),
        ))
        .mount(&mock)
        .await;

    let json = r#"[{"product":"P","component":"C","summary":"ok"},
                   {"product":"P","component":"Bad","summary":"bad"}]"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        matches!(
            &err,
            BzrError::BatchPartialFailure {
                succeeded: 1,
                failed: 1
            }
        ),
        "expected partial failure 1/1, got {err:?}"
    );
    assert_eq!(err.exit_code(), 11);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["created"], serde_json::json!([11]));
    assert_eq!(parsed["failed"][0]["index"], 1);
}

#[tokio::test]
async fn from_json_cli_flag_overrides_json_field() {
    let (_lock, mock, tmp) = setup_test_env().await;
    // The JSON says product "FromJson"; --product "FromCli" must win.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"product\":\"FromCli\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"product":"FromJson","component":"C","summary":"S"}"#;
    let mut action = from_json_action(&write_json_file(&tmp, json));
    if let BugAction::Create(crate::cli::CreateArgs { product, .. }) = &mut action {
        *product = Some("FromCli".into());
    }
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "CLI override should win and create: {result:?}"
    );
}

#[tokio::test]
async fn from_json_rejects_unknown_field() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"product":"P","component":"C","summary":"S","bogus":1}"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    match err {
        BzrError::InputValidation(msg) => assert!(
            msg.contains("bogus") || msg.contains("unknown field"),
            "should name the unknown field: {msg}"
        ),
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[tokio::test]
async fn from_json_missing_required_field_errors() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    // No summary in JSON and none on the CLI.
    let json = r#"{"product":"P","component":"C"}"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    match err {
        BzrError::InputValidation(msg) => assert!(msg.contains("summary"), "names field: {msg}"),
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[tokio::test]
async fn from_json_single_element_array_returns_batch_shape() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 8})))
        .expect(1)
        .mount(&mock)
        .await;

    // A top-level array of ONE must still yield the partial-failure shape
    // (`created`/`failed`), not the single-object `{id}` shape.
    let json = r#"[{"product":"P","component":"C","summary":"S"}]"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "1-element array should create: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["created"], serde_json::json!([8]));
    assert_eq!(parsed["failed"], serde_json::json!([]));
    assert!(
        parsed.get("id").is_none(),
        "array input must not use the single-object shape"
    );
}

#[tokio::test]
async fn from_json_batch_dry_run_emits_single_object_and_no_write() {
    let (_lock, mock, tmp) = setup_test_env().await;
    // No POST must fire under --dry-run.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let json = r#"[{"product":"P","component":"C","summary":"one"},
                   {"product":"P","component":"C","summary":"two"}]"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "batch dry-run should succeed: {result:?}");
    // The whole batch is ONE valid JSON object whose changes is the array.
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["changes"].as_array().unwrap().len(), 2);
}

#[test]
fn preview_params_propagates_editor_field_values() {
    // The editor preview is built from MergedFields::preview_params. Every
    // editor-visible field must flow into the CreateBugParams the editor
    // template renders; dropping any one would silently show the user a blank
    // field in the $EDITOR buffer.
    let merged = super::MergedFields {
        product: "Prod".into(),
        component: "Comp".into(),
        version: Some("9.9".into()),
        priority: Some("P1".into()),
        severity: Some("blocker".into()),
        assigned_to: Some("dev@example.com".into()),
        op_sys: Some("Linux".into()),
        rep_platform: Some("ARM".into()),
        url: Some("https://example.com/repro".into()),
        whiteboard: Some("needs-triage".into()),
        target_milestone: Some("M1".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["cc@example.com".into()],
        keywords: vec!["regression".into()],
        groups: vec!["security".into()],
        flags: vec![],
        template_description: None,
    };

    let p = merged.preview_params();

    assert_eq!(p.product, "Prod");
    assert_eq!(p.component, "Comp");
    assert_eq!(p.version, "9.9");
    assert_eq!(p.priority.as_deref(), Some("P1"));
    assert_eq!(p.severity.as_deref(), Some("blocker"));
    assert_eq!(p.assigned_to.as_deref(), Some("dev@example.com"));
    assert_eq!(p.op_sys.as_deref(), Some("Linux"));
    assert_eq!(p.rep_platform.as_deref(), Some("ARM"));
}

#[tokio::test]
async fn run_editor_flow_returns_parsed_editor_output() {
    // run_editor_flow renders the template, launches $EDITOR, and parses the
    // buffer back into (summary, description). The TTY gate that decides
    // *whether* to enter the editor lives a layer up in `handle`, so this
    // function is exercisable directly with a fake editor — no terminal needed.
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let script = install_fake_editor();
    let prev = std::env::var("EDITOR").ok();
    // SAFETY: setup_test_env holds bzr::ENV_LOCK for the test duration,
    // serializing env access across all tests that use it.
    unsafe { std::env::set_var("EDITOR", &script) };

    let merged = super::MergedFields {
        product: "Prod".into(),
        component: "Comp".into(),
        version: None,
        priority: None,
        severity: None,
        assigned_to: None,
        op_sys: None,
        rep_platform: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        cc: vec![],
        keywords: vec![],
        groups: vec![],
        flags: vec![],
        template_description: None,
    };
    let result = super::run_editor_flow(Some("Pre-fill"), &merged);

    // SAFETY: see above.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let _ = std::fs::remove_file(&script);

    let (summary, description) = result.unwrap();
    assert_eq!(summary, "Editor summary");
    assert_eq!(description, "Editor description");
}

#[test]
fn build_editor_template_separates_no_newline_body_from_sentinel() {
    use crate::types::CreateBugParams;
    let params = CreateBugParams {
        product: "P".into(),
        component: "C".into(),
        version: "1".into(),
        ..Default::default()
    };
    // The body has no trailing newline; the renderer must add one so a blank
    // line sits between the body and the sentinel. The `delete !` mutant drops
    // that, butting the body directly against the sentinel line.
    let buf = super::build_editor_template(None, Some("Body line"), &params);
    assert!(
        buf.contains("Body line\n\n"),
        "a blank line must follow a body lacking its own trailing newline, got: {buf:?}"
    );
}

#[tokio::test]
async fn from_json_cli_description_overrides_json_description() {
    // explicit_description resolves --description for the JSON path; a supplied
    // value must overwrite the JSON `description`. Mutants that always return
    // None / a constant would let the JSON value through (or inject garbage).
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"description\":\"cli-desc\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 9})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"product":"P","component":"C","summary":"S","description":"json-desc"}"#;
    let mut action = from_json_action(&write_json_file(&tmp, json));
    if let BugAction::Create(crate::cli::CreateArgs { description, .. }) = &mut action {
        *description = Some("cli-desc".into());
    }
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "CLI --description must override the JSON description on the wire: {result:?}"
    );
}

#[tokio::test]
async fn from_json_preserves_blocks_and_depends_on_without_cli_override() {
    // overlay_cli keeps the JSON `blocks`/`depends_on` when no --blocks/
    // --depends-on flag is supplied (the `!is_empty()` guards). The `delete !`
    // mutants invert that, clobbering the JSON arrays with the empty CLI vecs.
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_string_contains("\"blocks\":[10,20]"))
        .and(body_string_contains("\"depends_on\":[30]"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 12})))
        .expect(1)
        .mount(&mock)
        .await;

    let json =
        r#"{"product":"P","component":"C","summary":"S","blocks":[10,20],"depends_on":[30]}"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "JSON blocks/depends_on must reach the wire unchanged: {result:?}"
    );
}

#[tokio::test]
async fn from_json_batch_table_lists_created_ids() {
    // The table-mode batch summary prints "Created bugs: …" only when at least
    // one bug was created (`!created.is_empty()`). The `delete !` mutant inverts
    // the guard, suppressing the line on a successful batch.
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 11})))
        .expect(2)
        .mount(&mock)
        .await;

    let json = r#"[{"product":"P","component":"C","summary":"one"},
                   {"product":"P","component":"C","summary":"two"}]"#;
    let action = from_json_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "table batch create should succeed: {result:?}"
    );
    assert!(
        io.out_str().contains("Created bugs: #11"),
        "table output must list the created ids, got: {:?}",
        io.out_str()
    );
}

// ── Compound create (flag form) ──────────────────────────────────────

fn compound_args(
    with_attachment: Vec<std::path::PathBuf>,
    descriptions: Vec<&str>,
    with_comment: Option<&str>,
) -> crate::cli::CreateArgs {
    crate::cli::CreateArgs {
        from_json: None,
        template: None,
        product: Some("P".into()),
        component: Some("C".into()),
        summary: Some("S".into()),
        version: Some("unspecified".into()),
        description: Some("d".into()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
        with_comment: with_comment.map(Into::into),
        with_comment_file: None,
        with_attachment,
        attachment_description: descriptions.into_iter().map(Into::into).collect(),
        create_fields: crate::cli::CreateFieldArgs::default(),
    }
}

fn tmp_attachment(suffix: &str, body: &[u8]) -> tempfile::TempPath {
    let mut f = tempfile::Builder::new()
        .prefix("bzr-compound-")
        .suffix(suffix)
        .tempfile()
        .unwrap();
    f.write_all(body).unwrap();
    f.into_temp_path()
}

#[test]
fn build_compound_plan_more_descriptions_than_attachments_errors() {
    let att = tmp_attachment(".log", b"x");
    let args = compound_args(vec![att.to_path_buf()], vec!["d1", "d2"], None);
    let err = super::build_compound_plan(&args).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(matches!(err, BzrError::InputValidation(_)));
}

#[test]
fn build_compound_plan_description_without_attachment_errors() {
    let args = compound_args(vec![], vec!["orphan"], None);
    let err = super::build_compound_plan(&args).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn build_compound_plan_empty_comment_body_errors() {
    let args = compound_args(vec![], vec![], Some("   "));
    let err = super::build_compound_plan(&args).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn build_compound_plan_undescribed_attachment_defaults_summary_to_filename() {
    let a = tmp_attachment(".log", b"one");
    let b = tmp_attachment(".txt", b"two");
    let args = compound_args(
        vec![a.to_path_buf(), b.to_path_buf()],
        vec!["first only"],
        None,
    );
    let plan = super::build_compound_plan(&args).unwrap();
    assert_eq!(plan.attachments.len(), 2);
    assert_eq!(plan.attachments[0].summary, "first only");
    assert_eq!(plan.attachments[1].summary, plan.attachments[1].file_name);
}

#[tokio::test]
async fn compound_attachment_500_exits_11_with_id_on_stderr() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let att = tmp_attachment(".log", b"trace data");
    let file_name = att
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap()
        .to_string();
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 50})))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/50/attachment"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let action = BugAction::Create(compound_args(vec![att.to_path_buf()], vec!["trace"], None));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 11);
    assert!(io.err_str().contains("50"), "stderr: {}", io.err_str());
    assert!(
        io.err_str().contains(&file_name),
        "stderr should name the file: {}",
        io.err_str()
    );
}
