use crate::cli::ClassificationAction;
use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ClassificationAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let (client, _email) = super::shared::connect_client(server, api).await?;

    match action {
        ClassificationAction::View { name } => {
            let classification = client.get_classification(name).await?;
            output::print_classification(&classification, format);
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
    use crate::cli::ClassificationAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn classification_view_returns_data() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/classification/Unclassified"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "classifications": [{
                    "id": 1,
                    "name": "Unclassified",
                    "description": "Not yet classified",
                    "products": []
                }]
            })))
            .mount(&mock)
            .await;

        let action = ClassificationAction::View {
            name: "Unclassified".to_string(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
