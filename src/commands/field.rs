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
    let (client, _email) = super::shared::connect_client(server, api).await?;

    match action {
        FieldAction::List { name } => {
            let values = client.get_field_values(name).await?;
            output::print_field_values(&values, name, format);
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
    use crate::cli::FieldAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn field_list_returns_values() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

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
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
