use crate::cli::BugAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateBugParams, SearchParams, UpdateBugParams};

#[expect(
    clippy::too_many_lines,
    reason = "single match dispatch over many action variants"
)]
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        BugAction::List {
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
        } => {
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
        }
        BugAction::View {
            id,
            fields,
            exclude_fields,
        } => {
            let bug = client
                .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
                .await?;
            output::print_bug_detail(&bug, format);
        }
        BugAction::History { id, since } => {
            let history = client.get_bug_history_since(*id, since.as_deref()).await?;
            if history.is_empty() {
                #[expect(clippy::print_stdout)]
                {
                    println!("No history for bug #{id}.");
                }
            } else {
                output::print_history(&history, format);
            }
        }
        BugAction::Search {
            query,
            limit,
            fields,
            exclude_fields,
        } => {
            let params = SearchParams {
                quicksearch: Some(query.clone()),
                limit: Some(*limit),
                include_fields: fields.clone(),
                exclude_fields: exclude_fields.clone(),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format);
        }
        BugAction::Create {
            product,
            component,
            summary,
            version,
            description,
            priority,
            severity,
            assignee,
            op_sys,
            rep_platform,
        } => {
            let params = CreateBugParams {
                product: product.clone(),
                component: component.clone(),
                summary: summary.clone(),
                version: version.clone(),
                description: description.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                assigned_to: assignee.clone(),
                op_sys: op_sys.clone(),
                rep_platform: rep_platform.clone(),
            };
            let id = client.create_bug(&params).await?;
            output::print_result(
                &ActionResult::created(id, ResourceKind::Bug),
                &format!("Created bug #{id}"),
                format,
            );
        }
        BugAction::Update {
            id,
            status,
            resolution,
            assignee,
            priority,
            severity,
            summary,
            whiteboard,
            flag,
        } => {
            let flags = super::flags::parse_flags(flag)?;
            let params = UpdateBugParams {
                status: status.clone(),
                resolution: resolution.clone(),
                assigned_to: assignee.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                summary: summary.clone(),
                whiteboard: whiteboard.clone(),
                flags,
            };
            client.update_bug(*id, &params).await?;
            output::print_result(
                &ActionResult::updated(*id, ResourceKind::Bug),
                &format!("Updated bug #{id}"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, setup_test_env};
    use crate::cli::BugAction;
    use crate::types::OutputFormat;

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

        let action = BugAction::List {
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["summary"], "Test bug");
        assert_eq!(parsed[0]["status"], "NEW");
        assert_eq!(parsed[0]["product"], "TestProduct");
    }

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
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["summary"], "Test bug");
        assert_eq!(parsed["assigned_to"], "nobody@test.com");
    }

    #[tokio::test]
    async fn bug_update_sends_put() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("PUT"))
            .and(path("/rest/bug/42"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let action = BugAction::Update {
            id: 42,
            status: Some("RESOLVED".into()),
            resolution: Some("FIXED".into()),
            assignee: None,
            priority: None,
            severity: None,
            summary: None,
            whiteboard: None,
            flag: vec![],
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["action"], "updated");
        assert_eq!(parsed["id"], 42);
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

        let action = BugAction::Create {
            product: "TestProduct".into(),
            component: "General".into(),
            summary: "New bug".into(),
            version: "unspecified".into(),
            description: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["action"], "created");
        assert_eq!(parsed["id"], 99);
    }

    #[tokio::test]
    async fn bug_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = BugAction::List {
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
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
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not exist") || err.contains("101"),
            "expected not-found error, got: {err}"
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

        let action = BugAction::List {
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }
}
