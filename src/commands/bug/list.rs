use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::{OutputFormat, SearchParams};

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::List {
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        id,
        alias,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let params = SearchParams {
        product: product.clone(),
        component: component.clone(),
        status: status.clone(),
        assigned_to: assignee.clone(),
        creator: creator.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        id: id.clone(),
        alias: alias.clone(),
        limit: Some(*limit),
        include_fields: fields.clone(),
        exclude_fields: exclude_fields.clone(),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
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
            limit: 50,
            fields: None,
            exclude_fields: None,
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
        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["summary"], "Test bug");
        assert_eq!(parsed[0]["status"], "NEW");
        assert_eq!(parsed[0]["product"], "TestProduct");
    }

    #[tokio::test]
    async fn bug_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = empty_list_action();
        let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
    }

    #[tokio::test]
    async fn bug_list_malformed_json_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock)
            .await;

        let action = empty_list_action();
        let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }
}
