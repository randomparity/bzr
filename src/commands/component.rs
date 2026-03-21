use crate::cli::ComponentAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        ComponentAction::Create {
            product,
            name,
            description,
            default_assignee,
        } => {
            let params = CreateComponentParams {
                product: product.clone(),
                name: name.clone(),
                description: description.clone(),
                default_assignee: default_assignee.clone(),
            };
            let id = client.create_component(&params).await?;
            output::print_result(
                &ActionResult::created(id, ResourceKind::Component),
                &format!("Created component #{id} in product '{product}'"),
                format,
            );
        }
        ComponentAction::Update {
            id,
            name,
            description,
            default_assignee,
        } => {
            let params = UpdateComponentParams {
                name: name.clone(),
                description: description.clone(),
                default_assignee: default_assignee.clone(),
            };
            client.update_component(*id, &params).await?;
            output::print_result(
                &ActionResult::updated(*id, ResourceKind::Component),
                &format!("Updated component #{id}"),
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

    use super::super::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::cli::ComponentAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn component_create_succeeds() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("POST"))
            .and(path("/rest/component"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
            .mount(&mock)
            .await;

        let action = ComponentAction::Create {
            product: "TestProduct".to_string(),
            name: "Backend".to_string(),
            description: "Backend component".to_string(),
            default_assignee: "dev@test.com".to_string(),
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed["id"], 42);
    }

    #[tokio::test]
    async fn component_create_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("POST"))
            .and(path("/rest/component"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = ComponentAction::Create {
            product: "TestProduct".to_string(),
            name: "Backend".to_string(),
            description: "Backend component".to_string(),
            default_assignee: "dev@test.com".to_string(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn component_update_succeeds() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("PUT"))
            .and(path("/rest/component/10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
            .mount(&mock)
            .await;

        let action = ComponentAction::Update {
            id: 10,
            name: Some("Updated".to_string()),
            description: None,
            default_assignee: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed["id"], 10);
        assert_eq!(parsed["action"], "updated");
    }
}
