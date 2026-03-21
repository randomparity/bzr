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
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, setup_test_env};
    use crate::cli::ServerAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn server_info_returns_version_and_extensions() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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

        let (result, output) = capture_stdout(super::execute(
            &ServerAction::Info,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["version"], "5.0.4");
    }

    #[tokio::test]
    async fn server_info_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let result = super::execute(&ServerAction::Info, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
    }
}
