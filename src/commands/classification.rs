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
    let client = super::shared::connect_client(server, api).await?;

    match action {
        ClassificationAction::View { name } => {
            let classification = client.get_classification(name).await?;
            output::print_classification(&classification, format);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::cli::ClassificationAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn classification_view_returns_data() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed["name"], "Unclassified");
        assert_eq!(parsed["description"], "Not yet classified");
    }

    #[tokio::test]
    async fn classification_view_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/classification/Missing"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = ClassificationAction::View {
            name: "Missing".to_string(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }
}
