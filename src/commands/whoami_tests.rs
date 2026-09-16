use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::test_helpers::setup_isolated_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn whoami_returns_user_info() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;

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

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io.writers(),
    )
    .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["name"], "admin@test.com");
    assert_eq!(parsed["real_name"], "Admin User");
    // Connection metadata composed locally, not from the server body.
    assert_eq!(parsed["server_name"], "test");
    assert_eq!(parsed["auth_mode"], "api_key");
}

#[tokio::test]
async fn whoami_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let result = super::execute(
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
