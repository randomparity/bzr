use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
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
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
    assert!(
        output.contains("No history for bug #42."),
        "expected empty-history message, got: {output}"
    );
}
