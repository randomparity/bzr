use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;

pub async fn execute(
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;
    let whoami = client.whoami().await?;
    output::print_whoami(&whoami, format);
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::await_holding_lock)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn whoami_returns_user_info() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "name": "admin@test.com",
                "real_name": "Admin User"
            })))
            .mount(&mock)
            .await;

        let result = super::execute(None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
