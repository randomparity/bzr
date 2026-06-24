#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn clone_args(id: &str) -> crate::cli::CloneArgs {
    crate::cli::CloneArgs {
        id: id.to_string(),
        summary: None,
        product: None,
        component: None,
        version: None,
        description: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        create_fields: crate::cli::CloneCreateFieldArgs::default(),
        no_comment: false,
        add_depends_on: false,
        add_blocks: false,
        no_cc: false,
        no_keywords: false,
    }
}

#[tokio::test]
async fn bug_clone_copies_fields() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Mock get_bug
    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "2.0",
                "priority": "P1",
                "severity": "major",
                "assigned_to": "dev@test.com",
                "op_sys": "Linux",
                "rep_platform": "x86_64",
                "cc": ["watcher@test.com"],
                "keywords": ["regression"]
            }]
        })))
        .mount(&mock)
        .await;

    // The cloned description must come from comment count=0, not a
    // follow-up comment.
    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "100": {
                    "comments": [
                        {
                            "id": 2,
                            "bug_id": 100,
                            "count": 1,
                            "text": "Follow-up reply",
                            "creator": "dev@test.com",
                            "creation_time": "2025-01-02T00:00:00Z"
                        },
                        {
                            "id": 1,
                            "bug_id": 100,
                            "count": 0,
                            "text": "Original description",
                            "creator": "dev@test.com",
                            "creation_time": "2025-01-01T00:00:00Z"
                        }
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .and(body_partial_json(serde_json::json!({
            "description": "Original description"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 200})))
        .expect(1)
        .mount(&mock)
        .await;

    // Mock add_comment (for "Cloned from" comment)
    Mock::given(method("POST"))
        .and(path("/rest/bug/200/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 300})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Clone(clone_args("100"));
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "bug clone failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 200);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn bug_clone_reports_id_when_comment_post_fails() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "1.0",
                "cc": [],
                "keywords": []
            }]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "100": { "comments": [{
                "id": 1, "bug_id": 100, "count": 0, "text": "Description",
                "creator": "dev@test.com", "creation_time": "2025-01-01T00:00:00Z"
            }] } }
        })))
        .mount(&mock)
        .await;

    // Bug creation succeeds.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 202})))
        .expect(1)
        .mount(&mock)
        .await;

    // ...but the "Cloned from" comment POST fails with a 500.
    Mock::given(method("POST"))
        .and(path("/rest/bug/202/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Clone(clone_args("100"));
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;

    // The clone (bug creation) succeeded, so the command must succeed and the
    // new bug ID must be reported — otherwise the user can't tell a bug was
    // created and may re-clone, producing a duplicate.
    assert!(
        result.is_ok(),
        "clone should succeed despite comment failure: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(parsed["id"], 202);
    assert_eq!(parsed["action"], "created");
    // The comment failure is surfaced as a stderr warning naming the new ID.
    let err = __io.err_str();
    assert!(
        err.contains("202") && err.contains("warning"),
        "expected warning naming bug #202, got: {err:?}"
    );
}

#[tokio::test]
async fn bug_clone_no_comment_skips_comment() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "1.0",
                "cc": [],
                "keywords": []
            }]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "100": {
                    "comments": [{
                        "id": 1,
                        "bug_id": 100,
                        "count": 0,
                        "text": "Description",
                        "creator": "dev@test.com",
                        "creation_time": "2025-01-01T00:00:00Z"
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 201})))
        .expect(1)
        .mount(&mock)
        .await;

    // No comment mock — if comment is posted, the test will fail because there's no mock
    Mock::given(method("POST"))
        .and(path("/rest/bug/201/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 301})))
        .expect(0)
        .mount(&mock)
        .await;

    let mut args = clone_args("100");
    args.no_comment = true;
    let action = BugAction::Clone(args);
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io2.writers(),
    )
    .await;
    let _output = __io2.out_str().to_string();
    assert!(result.is_ok(), "bug clone --no-comment failed: {result:?}");
}

#[tokio::test]
async fn bug_clone_dry_run_reads_source_but_creates_nothing() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Source fetch (GET) is allowed in a dry run; it builds the would-be payload.
    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "2.0",
                "priority": "P1",
                "severity": "major",
                "assigned_to": "dev@test.com",
                "op_sys": "Linux",
                "rep_platform": "x86_64",
                "url": "https://example.com/source",
                "whiteboard": "needs-triage",
                "target_milestone": "M2",
                "deadline": "2026-12-31",
                "cc": ["watcher@test.com"],
                "keywords": ["regression"]
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "100": { "comments": [{
                "id": 1, "bug_id": 100, "count": 0, "text": "Original description",
                "creator": "dev@test.com", "creation_time": "2025-01-01T00:00:00Z"
            }] } }
        })))
        .mount(&mock)
        .await;
    // No write may happen: any create POST fails the test on drop.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::Clone(clone_args("100"));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run clone failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["product"], "TestProduct");
    assert_eq!(parsed["changes"]["summary"], "Original bug");
    // Each cloned field must be carried into the payload — a dropped field
    // would fall back to its Default (empty/None) and disappear here.
    assert_eq!(parsed["changes"]["component"], "General");
    assert_eq!(parsed["changes"]["version"], "2.0");
    assert_eq!(parsed["changes"]["priority"], "P1");
    assert_eq!(parsed["changes"]["severity"], "major");
    assert_eq!(parsed["changes"]["assigned_to"], "dev@test.com");
    assert_eq!(parsed["changes"]["op_sys"], "Linux");
    assert_eq!(parsed["changes"]["rep_platform"], "x86_64");
    assert_eq!(parsed["changes"]["url"], "https://example.com/source");
    assert_eq!(parsed["changes"]["whiteboard"], "needs-triage");
    assert_eq!(parsed["changes"]["target_milestone"], "M2");
    assert_eq!(parsed["changes"]["deadline"], "2026-12-31");
    assert_eq!(
        parsed["changes"]["cc"],
        serde_json::json!(["watcher@test.com"])
    );
    assert_eq!(
        parsed["changes"]["keywords"],
        serde_json::json!(["regression"])
    );
}

#[tokio::test]
async fn bug_clone_dry_run_links_blocks_and_depends_on_to_source() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "2.0",
                "cc": [],
                "keywords": []
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "100": { "comments": [{
                "id": 1, "bug_id": 100, "count": 0, "text": "Original description",
                "creator": "dev@test.com", "creation_time": "2025-01-01T00:00:00Z"
            }] } }
        })))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(0)
        .mount(&mock)
        .await;

    // --add-blocks / --add-depends-on make the new bug block and depend on the
    // source. Without these the lists are empty (and serialization omits them),
    // so a dropped `blocks`/`depends_on` field is only observable here.
    let mut args = clone_args("100");
    args.add_blocks = true;
    args.add_depends_on = true;
    let action = BugAction::Clone(args);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run clone failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["changes"]["blocks"], serde_json::json!([100]));
    assert_eq!(parsed["changes"]["depends_on"], serde_json::json!([100]));
}

#[tokio::test]
async fn bug_clone_dry_run_applies_create_metadata_overrides() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 100,
                "summary": "Original bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General",
                "version": "2.0",
                "url": "https://example.com/source",
                "whiteboard": "source-whiteboard",
                "target_milestone": "M2",
                "deadline": "2026-12-30",
                "cc": ["source-watcher@test.com"],
                "keywords": ["source-keyword"]
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/100/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "100": { "comments": [{
                "id": 1, "bug_id": 100, "count": 0, "text": "Original description",
                "creator": "dev@test.com", "creation_time": "2025-01-01T00:00:00Z"
            }] } }
        })))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(0)
        .mount(&mock)
        .await;

    let mut args = clone_args("100");
    args.create_fields = crate::cli::CloneCreateFieldArgs {
        url: Some("https://example.com/override".into()),
        whiteboard: Some("override-whiteboard".into()),
        target_milestone: Some("M3".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["override-watcher@test.com".into()],
        keywords: vec!["override-keyword".into()],
        groups: vec!["confidential".into()],
        flag: vec!["review?(qa@example.com)".into()],
    };
    let action = BugAction::Clone(args);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run clone failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    let changes = &parsed["changes"];
    assert_eq!(changes["url"], "https://example.com/override");
    assert_eq!(changes["whiteboard"], "override-whiteboard");
    assert_eq!(changes["target_milestone"], "M3");
    assert_eq!(changes["deadline"], "2026-12-31");
    assert_eq!(
        changes["cc"],
        serde_json::json!(["override-watcher@test.com"])
    );
    assert_eq!(changes["keywords"], serde_json::json!(["override-keyword"]));
    assert_eq!(changes["groups"], serde_json::json!(["confidential"]));
    assert_eq!(changes["flags"][0]["name"], "review");
    assert_eq!(changes["flags"][0]["status"], "?");
    assert_eq!(changes["flags"][0]["requestee"], "qa@example.com");
}
