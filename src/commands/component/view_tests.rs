use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ComponentAction;
use crate::error::BzrError;
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
async fn component_view_returns_one_component() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::View {
        product: "MyApp".into(),
        name: "Frontend".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "view should succeed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed["id"], 11);
    assert_eq!(parsed["name"], "Frontend");
}

#[tokio::test]
async fn component_view_unknown_name_is_not_found() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::View {
        product: "MyApp".into(),
        name: "Nonexistent".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(BzrError::NotFound {
            resource: "component",
            ..
        })
    ));
}
