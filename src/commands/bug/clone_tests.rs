#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

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
                            "count": 1,
                            "text": "Follow-up reply",
                            "creator": "dev@test.com",
                            "creation_time": "2025-01-02T00:00:00Z"
                        },
                        {
                            "id": 1,
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

    let action = BugAction::Clone {
        id: "100".to_string(),
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
        no_comment: false,
        add_depends_on: false,
        add_blocks: false,
        no_cc: false,
        no_keywords: false,
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
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
                "id": 1, "count": 0, "text": "Description",
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

    let action = BugAction::Clone {
        id: "100".to_string(),
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
        no_comment: false,
        add_depends_on: false,
        add_blocks: false,
        no_cc: false,
        no_keywords: false,
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
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

    let action = BugAction::Clone {
        id: "100".to_string(),
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
        no_comment: true,
        add_depends_on: false,
        add_blocks: false,
        no_cc: false,
        no_keywords: false,
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
    assert!(result.is_ok(), "bug clone --no-comment failed: {result:?}");
}

#[tokio::test]
async fn bug_clone_dry_run_reads_source_but_creates_nothing() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    crate::commands::dry_run::set(true);

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
                "id": 1, "count": 0, "text": "Original description",
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

    let action = BugAction::Clone {
        id: "100".to_string(),
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
        no_comment: false,
        add_depends_on: false,
        add_blocks: false,
        no_cc: false,
        no_keywords: false,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;
    let output = io.out_str().to_string();
    crate::commands::dry_run::set(false);

    assert!(result.is_ok(), "dry-run clone failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["product"], "TestProduct");
    assert_eq!(parsed["changes"]["summary"], "Original bug");
}
