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
#[expect(clippy::unwrap_used, clippy::await_holding_lock)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::ComponentAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn component_create_succeeds() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

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
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
