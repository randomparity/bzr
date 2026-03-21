use crate::cli::FieldAction;
use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &FieldAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        FieldAction::List { name } => {
            let values = client.get_field_values(name).await?;
            if values.is_empty() && format == OutputFormat::Table {
                #[expect(clippy::print_stdout)]
                {
                    println!("No values for field '{name}'.");
                }
            } else {
                output::print_field_values(&values, format);
            }
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
    use crate::cli::FieldAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn field_list_returns_values() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/field/bug/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "fields": [{
                    "name": "status",
                    "values": [
                        {"name": "NEW"},
                        {"name": "ASSIGNED"},
                        {"name": "RESOLVED"}
                    ]
                }]
            })))
            .mount(&mock)
            .await;

        let action = FieldAction::List {
            name: "status".to_string(),
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert!(parsed.as_array().unwrap().len() >= 3);
        assert_eq!(parsed[0]["name"], "NEW");
    }

    #[tokio::test]
    async fn field_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/field/bug/status"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = FieldAction::List {
            name: "status".to_string(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }
}
