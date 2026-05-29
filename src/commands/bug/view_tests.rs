#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn make_view_action(ids: &[&str], permissive: bool) -> BugAction {
    BugAction::View {
        ids: ids.iter().map(|s| (*s).to_string()).collect(),
        permissive,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
    }
}

fn ok_bug_body(id: u64, summary: &str) -> serde_json::Value {
    serde_json::json!({
        "bugs": [{
            "id": id,
            "summary": summary,
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
    })
}

fn api_error_body(code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": true,
        "code": code,
        "message": message
    })
}

#[tokio::test]
async fn view_single_unchanged_table() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(42, "Test bug")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["42"], false);
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "{:?}", result.err());
    assert!(output.contains("Bug #42"));
    assert!(output.contains("Test bug"));
    // Single-ID → no divider.
    assert!(!output.contains(&"─".repeat(60)));
}

#[tokio::test]
async fn view_single_unchanged_json() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(42, "Test bug")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["42"], false);
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;
    let output = __io2.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    // Single-ID JSON shape must remain a bare Bug object — no `bugs`/`failed`.
    assert_eq!(parsed["id"], 42);
    assert!(parsed.get("bugs").is_none());
    assert!(parsed.get("failed").is_none());
}

#[tokio::test]
async fn view_single_failure_propagates() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/999999"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(api_error_body(101, "Bug #999999 does not exist.")),
        )
        .mount(&mock)
        .await;

    let action = make_view_action(&["999999"], false);
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
        err.contains("does not exist") || err.contains("101"),
        "expected not-found error message, got: {err}"
    );
}

#[tokio::test]
async fn view_multi_strict_all_succeed_table() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    for (id, summary) in [(1, "first"), (2, "second"), (3, "third")] {
        Mock::given(method("GET"))
            .and(path(format!("/rest/bug/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(id, summary)))
            .mount(&mock)
            .await;
    }

    let action = make_view_action(&["1", "2", "3"], false);
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io3.writers(),
    )
    .await;
    let output = __io3.out_str().to_string();
    assert!(result.is_ok());
    assert!(output.contains("Bug #1"));
    assert!(output.contains("Bug #2"));
    assert!(output.contains("Bug #3"));
    // Three bugs ⇒ exactly two dividers.
    assert_eq!(output.matches(&"─".repeat(60)).count(), 2);
}

#[tokio::test]
async fn view_multi_strict_failure_emits_no_partial_table() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(101, "Bug #2 does not exist.")),
        )
        .mount(&mock)
        .await;
    // No mock for /rest/bug/3 — if the loop calls it, wiremock returns 404
    // with no body, which would surface as a transport error. The test
    // confirms the loop never gets that far.

    let action = make_view_action(&["1", "2", "3"], false);
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io4.writers(),
    )
    .await;
    let output = __io4.out_str().to_string();
    assert!(result.is_err());
    assert!(
        output.trim().is_empty(),
        "strict multi-ID table output should be all-or-nothing, got: {output}"
    );
    // Bug #3 must NOT have been fetched after bug #2 failed.
    assert!(!output.contains("Bug #3"));
}

#[tokio::test]
async fn view_multi_strict_json_all_succeed_emits_wrapped_shape() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    for (id, summary) in [(1, "first"), (2, "second")] {
        Mock::given(method("GET"))
            .and(path(format!("/rest/bug/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(id, summary)))
            .mount(&mock)
            .await;
    }

    let action = make_view_action(&["1", "2"], false);
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["bugs"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["failed"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["bugs"][0]["id"], 1);
    assert_eq!(parsed["bugs"][1]["id"], 2);
}

#[tokio::test]
async fn view_multi_strict_json_failure_emits_no_partial_json() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(101, "Bug #2 does not exist.")),
        )
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2"], false);
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
    assert!(result.is_err());
    // No JSON should have been emitted — the buffer was discarded on Err.
    // CapturedIo gives us a per-test owned buffer, so a strict byte-empty
    // assertion is now correct (no more concurrent-writer contamination).
    assert!(
        output.trim().is_empty(),
        "expected empty stdout, got: {output}"
    );
}

#[tokio::test]
async fn view_multi_permissive_partial_table() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(101, "Bug #2 does not exist.")),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(3, "third")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2", "3"], true);
    let mut __io7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io7.writers(),
    )
    .await;
    let output = __io7.out_str().to_string();
    assert!(
        result.is_ok(),
        "permissive must exit Ok, got: {:?}",
        result.err()
    );
    assert!(output.contains("Bug #1"));
    assert!(output.contains("Bug #2"));
    assert!(output.contains("UNAVAILABLE"));
    assert!(output.contains("Bug #3"));
    // Argument order preserved.
    let pos1 = output.find("Bug #1").unwrap();
    let pos2 = output.find("Bug #2").unwrap();
    let pos3 = output.find("Bug #3").unwrap();
    assert!(pos1 < pos2 && pos2 < pos3);
}

#[tokio::test]
async fn view_multi_permissive_json_shape() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(101, "Bug #2 does not exist.")),
        )
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2"], true);
    let mut __io8 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io8.writers(),
    )
    .await;
    let output = __io8.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["bugs"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["bugs"][0]["id"], 1);
    let failed = parsed["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], "2");
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("does not exist")
            || failed[0]["error"].as_str().unwrap().contains("101")
    );
}

#[tokio::test]
async fn view_multi_permissive_all_fail_returns_empty_bugs() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    for id in [1_u64, 2, 3] {
        Mock::given(method("GET"))
            .and(path(format!("/rest/bug/{id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(api_error_body(101, "no such bug")),
            )
            .mount(&mock)
            .await;
    }

    let action = make_view_action(&["1", "2", "3"], true);
    let mut __io9 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io9.writers(),
    )
    .await;
    let output = __io9.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["bugs"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["failed"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn view_multi_permissive_with_alias_preserves_id_string() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/my-alias"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(100, "Invalid Bug Alias")),
        )
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "my-alias"], true);
    let mut __io10 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io10.writers(),
    )
    .await;
    let output = __io10.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    let failed = parsed["failed"].as_array().unwrap();
    assert_eq!(
        failed[0]["id"], "my-alias",
        "alias preserved verbatim in failure id"
    );
}

#[tokio::test]
async fn view_permissive_single_id_rejected() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = make_view_action(&["42"], true);
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
    assert_eq!(err.exit_code(), 7);
    assert!(err
        .to_string()
        .contains("only meaningful with multiple IDs"));
}

#[tokio::test]
async fn view_multi_permissive_transport_error_bails() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(500).set_body_string(""))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(3, "third")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2", "3"], true);
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_err(),
        "transport error must bail despite --permissive"
    );
    // It must be a transport-flavored error, not a per-resource one.
    assert!(!result.unwrap_err().is_bug_get_per_resource());
}

#[tokio::test]
async fn view_multi_permissive_api_session_wide_bails() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    // Code 32000 (Login Required) is session-wide — must bail.
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(32000, "Login Required")),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(3, "third")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2", "3"], true);
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_err(),
        "session-wide Api code must bail despite --permissive"
    );
}

#[tokio::test]
async fn view_multi_permissive_api_102_suppressed() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(api_error_body(102, "Access Denied")),
        )
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2"], true);
    let mut __io11 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io11.writers(),
    )
    .await;
    let output = __io11.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["failed"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["failed"][0]["id"], "2");
}

#[tokio::test]
async fn view_multi_permissive_api_410_bails() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    // Code 410 is NOT in the Bug.get per-bug whitelist (100/101/102) —
    // it's a session-adjacent auth/permission failure. Must bail under
    // `--permissive` just like a transport error or session-wide code.
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(api_error_body(
            410,
            "You must log in to access this resource.",
        )))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_bug_body(3, "third")))
        .mount(&mock)
        .await;

    let action = make_view_action(&["1", "2", "3"], true);
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_err(),
        "auth-flavored Api code must bail despite --permissive"
    );
    assert!(!result.unwrap_err().is_bug_get_per_resource());
}
