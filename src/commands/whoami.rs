//! Whoami command — shows the authenticated user's identity.

use crate::cli::WhoamiAction;
use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    _action: &WhoamiAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;
    let whoami = client.whoami().await?;
    output::print_whoami(&whoami, format);
    Ok(())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn whoami_returns_user_info() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "name": "admin@test.com",
                "real_name": "Admin User"
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let (result, output) = capture_stdout(super::execute(
            &crate::cli::WhoamiAction::Show,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["name"], "admin@test.com");
        assert_eq!(parsed["real_name"], "Admin User");
    }

    #[tokio::test]
    async fn whoami_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let result = super::execute(
            &crate::cli::WhoamiAction::Show,
            None,
            OutputFormat::Json,
            None,
        )
        .await;
        assert!(result.is_err());
    }
}
