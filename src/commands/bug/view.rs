use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::View {
        id,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let bug = client
        .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
        .await?;
    output::print_bug_detail(&bug, format);
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use crate::cli::BugAction;
    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn bug_view_returns_detail() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
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

        let action = BugAction::View {
            id: "42".to_string(),
            fields: None,
            exclude_fields: None,
        };
        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["summary"], "Test bug");
        assert_eq!(parsed["assigned_to"], "nobody@test.com");
    }

    #[tokio::test]
    async fn bug_view_not_found_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/999999"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 101,
                "message": "Bug #999999 does not exist."
            })))
            .mount(&mock)
            .await;

        let action = BugAction::View {
            id: "999999".to_string(),
            fields: None,
            exclude_fields: None,
        };
        let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not exist") || err.contains("101"),
            "expected not-found error, got: {err}"
        );
    }
}
