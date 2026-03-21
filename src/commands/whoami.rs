//! Whoami command — shows the authenticated user's identity.
//!
//! Unlike other command modules, `execute()` has no action enum parameter
//! because `whoami` has no subcommands.

use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let (client, email_hint) = super::shared::connect_client_with_email(server, api).await?;
    // Email hint is used for Bugzilla 5.0 fallback (whoami endpoint
    // was added in 5.1; older servers need a user lookup by email).
    let whoami = client.whoami(email_hint.as_deref()).await?;
    output::print_whoami(&whoami, format);
    Ok(())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::setup_test_env;
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

        let result = super::execute(None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
