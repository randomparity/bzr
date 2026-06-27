#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{ProductAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_one_product_detail(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1, "name": "Firefox", "description": "Web browser",
                "is_active": true, "components": [], "versions": [], "milestones": []
            }]
        })))
        .mount(mock)
        .await;
}

fn view_with(projection: ProjectionArgs) -> ProductAction {
    ProductAction::View {
        name: "Firefox".to_string(),
        projection,
    }
}

#[tokio::test]
async fn product_view_returns_detail() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "Firefox",
                "description": "Web browser",
                "is_active": true,
                "components": [],
                "versions": [],
                "milestones": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = ProductAction::View {
        name: "Firefox".to_string(),
        projection: ProjectionArgs::default(),
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "Firefox");
    assert_eq!(parsed["description"], "Web browser");
}

#[tokio::test]
async fn product_view_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ProductAction::View {
        name: "Firefox".to_string(),
        projection: ProjectionArgs::default(),
    };
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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

#[tokio::test]
async fn product_view_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_product_detail(&mock).await;

    let action = view_with(ProjectionArgs {
        fields: Some("id".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed.as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn product_view_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = view_with(ProjectionArgs {
        fields: Some("nam".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(io.out_str().is_empty());
}
