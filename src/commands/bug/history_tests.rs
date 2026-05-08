#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn bug_history_empty_prints_no_history_message() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{ "id": 42, "history": [] }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::History {
        id: 42,
        since: None,
    };
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
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
    assert!(
        output.contains("No history for bug #42."),
        "expected empty-history message, got: {output}"
    );
}

#[tokio::test]
async fn bug_history_rejects_malformed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::History {
        id: 42,
        since: Some("yesterday".into()),
    };
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(msg.contains("--since"), "error should name the flag: {msg}");
    assert!(
        msg.contains("yesterday"),
        "error should echo the offending input: {msg}"
    );
}
