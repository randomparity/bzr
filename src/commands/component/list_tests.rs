#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ComponentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

/// Mock `GET /rest/product?names=MyApp` returning a product with two
/// components.
async fn mount_product_with_components(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .and(query_param("names", "MyApp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "MyApp",
                "description": "App",
                "is_active": true,
                "components": [
                    {"id": 10, "name": "Backend", "description": "be", "is_active": true,
                     "default_assignee": "dev@example.com"},
                    {"id": 11, "name": "Frontend", "description": "fe", "is_active": false,
                     "default_assignee": null}
                ],
                "versions": [],
                "milestones": []
            }]
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn component_list_returns_components() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::List {
        product: "MyApp".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "list should succeed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["name"], "Backend");
}

#[tokio::test]
async fn component_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ComponentAction::List {
        product: "MyApp".into(),
    };
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}
