#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn empty_list_action() -> BugAction {
    BugAction::List {
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        id: vec![],
        alias: None,
        summary: None,
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        created_since: None,
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    }
}

#[tokio::test]
async fn bug_list_returns_bugs() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 1,
                "summary": "Test bug",
                "status": "NEW",
                "resolution": "",
                "assigned_to": "nobody@test.com",
                "priority": "P1",
                "severity": "normal",
                "product": "TestProduct",
                "component": "General",
                "creation_time": "2025-01-01T00:00:00Z",
                "last_change_time": "2025-01-01T00:00:00Z"
            }]
        })))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["summary"], "Test bug");
    assert_eq!(parsed[0]["status"], "NEW");
    assert_eq!(parsed[0]["product"], "TestProduct");
}

#[tokio::test]
async fn bug_list_passes_every_field_through_to_search_params() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    // Every CLI field on `bug list` must round-trip into the search
    // query string.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "Firefox"))
        .and(query_param("component", "General"))
        .and(query_param("status", "NEW"))
        .and(query_param("assigned_to", "dev@test.com"))
        .and(query_param("creator", "reporter@test.com"))
        .and(query_param("priority", "P1"))
        .and(query_param("severity", "major"))
        .and(query_param("id", "42"))
        .and(query_param("alias", "my-alias"))
        .and(query_param("summary", "kernel panic"))
        .and(query_param("limit", "5"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "comments"))
        .and(query_param("creation_time", "2026-04-01T00:00:00Z"))
        .and(query_param("last_change_time", "2026-04-15T00:00:00Z"))
        .and(query_param("whiteboard", "needs-review"))
        .and(query_param("target_milestone", "5.0"))
        .and(query_param("version", "9.4"))
        .and(query_param("op_sys", "Linux"))
        .and(query_param("platform", "x86_64"))
        .and(query_param("resolution", "FIXED"))
        .and(query_param("qa_contact", "qa@test.com"))
        .and(query_param("url", "github.com/foo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::List {
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assignee: vec!["dev@test.com".into()],
        creator: vec!["reporter@test.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["major".into()],
        id: vec![42],
        alias: Some("my-alias".into()),
        summary: Some("kernel panic".into()),
        limit: 5,
        field_args: crate::cli::FieldArgs {
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
        },
        created_since: Some("2026-04-01".into()),
        changed_since: Some("2026-04-15T00:00:00Z".into()),
        whiteboard: vec!["needs-review".into()],
        target_milestone: vec!["5.0".into()],
        version: vec!["9.4".into()],
        op_sys: vec!["Linux".into()],
        platform: vec!["x86_64".into()],
        resolution: vec!["FIXED".into()],
        qa_contact: vec!["qa@test.com".into()],
        url: vec!["github.com/foo".into()],
    };
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "bug list with all fields failed: {result:?}"
    );
}

#[tokio::test]
async fn bug_list_summary_only_sends_substring_filter() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    // `--summary` alone must be passed verbatim as the REST `summary`
    // query parameter and must not trigger an XML-RPC fallback even
    // when the result is empty (issue #152).
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("summary", "WARNING CPU default_machine_kexec"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::List {
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        id: vec![],
        alias: None,
        summary: Some("WARNING CPU default_machine_kexec".into()),
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        created_since: None,
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "bug list --summary failed: {result:?}");
}

#[tokio::test]
async fn bug_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn bug_list_malformed_json_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn bug_list_rejects_malformed_created_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    if let BugAction::List { created_since, .. } = &mut action {
        *created_since = Some("not-a-date".into());
    }

    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "expected exit code 7 for input validation, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("--created-since"),
        "error should name the flag: {msg}"
    );
    assert!(
        msg.contains("not-a-date"),
        "error should echo the offending input: {msg}"
    );
}

#[tokio::test]
async fn bug_list_rejects_malformed_changed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    if let BugAction::List { changed_since, .. } = &mut action {
        *changed_since = Some("2026-13-99".into());
    }

    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--changed-since"));
}

#[tokio::test]
async fn bug_list_mixed_positive_notequals_notsubstring() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    // End-to-end coverage: --product (positive), --resolution '!FIXED'
    // (notequals), --whiteboard '!wip' (notsubstring) all reach the
    // wire with the right operator.
    let (_lock, mock, _tmp) = setup_test_env().await;

    // FIELD_MAPPINGS iterates whiteboard (idx 7) before resolution
    // (idx 12), so whiteboard gets f1 and resolution gets f2.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "P"))
        .and(query_param("f1", "status_whiteboard"))
        .and(query_param("o1", "notsubstring"))
        .and(query_param("v1", "wip"))
        .and(query_param("f2", "resolution"))
        .and(query_param("o2", "notequals"))
        .and(query_param("v2", "FIXED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let mut action = empty_list_action();
    if let BugAction::List {
        product,
        resolution,
        whiteboard,
        ..
    } = &mut action
    {
        *product = vec!["P".into()];
        *resolution = vec!["!FIXED".into()];
        *whiteboard = vec!["!wip".into()];
    }
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "bug list failed: {result:?}");
}

#[tokio::test]
async fn bug_list_table_all_unknown_fields_exits_7_before_network() {
    // No /rest/bug mock is mounted: the field selection must be rejected
    // (exit 7) before any network I/O at all (F2). The validation is
    // hoisted ahead of `connect_and_configure`, so not even the auth/TLS
    // probe round-trip fires — the mock sees zero requests.
    let (_lock, mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    let BugAction::List { field_args, .. } = &mut action else {
        unreachable!()
    };
    field_args.fields = Some("cf_custom".into());

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7, "all-unknown --fields exits 7: {err}");
    assert!(
        mock.received_requests().await.unwrap().is_empty(),
        "validation must run before any network I/O"
    );
}

#[tokio::test]
async fn bug_list_json_fields_trims_output() {
    // --json with a field selection trims the output object to the selected
    // fields (gh-style): `--fields summary` yields `{"summary": ...}` only,
    // with id and every other unselected key dropped.
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "Test bug", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;

    let mut action = empty_list_action();
    let BugAction::List { field_args, .. } = &mut action else {
        unreachable!()
    };
    field_args.fields = Some("summary".into());

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    assert!(result.is_ok(), "json path succeeds: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    let obj = parsed[0].as_object().unwrap();
    assert!(
        obj.contains_key("summary"),
        "summary present:\n{}",
        __io.out_str()
    );
    assert!(
        !obj.contains_key("status") && !obj.contains_key("id"),
        "unselected keys trimmed:\n{}",
        __io.out_str()
    );
}

#[tokio::test]
async fn bug_list_json_without_fields_does_not_warn() {
    // --json with no field selection: full object, no unknown-field warning.
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "Test bug", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    assert!(result.is_ok(), "json list succeeds: {result:?}");
    assert!(
        __io.err_str().is_empty(),
        "no warning without a field selection: {:?}",
        __io.err_str()
    );
}

#[tokio::test]
async fn bug_list_json_all_unknown_fields_exits_7() {
    // --json validation measures emptiness against the full field universe: an
    // all-unknown --fields value exits 7 before any network I/O, mirroring
    // table mode. (`bug view` stays exempt — covered in the integration suite.)
    let (_lock, mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    let BugAction::List { field_args, .. } = &mut action else {
        unreachable!()
    };
    field_args.fields = Some("cf_custom".into());

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "all-unknown --fields under --json exits 7: {err}"
    );
    assert!(
        mock.received_requests().await.unwrap().is_empty(),
        "validation must run before any network I/O"
    );
}
