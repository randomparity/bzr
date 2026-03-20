use crate::cli::ServerAction;
use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ServerAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        ServerAction::Info => {
            let info = client.server_info().await?;
            output::print_server_info(&output::ServerInfo::from(&info), format);
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::ServerAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn server_info_returns_version_and_extensions() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
            )
            .mount(&mock)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/extensions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"extensions": {}})),
            )
            .mount(&mock)
            .await;

        let result = super::execute(&ServerAction::Info, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
