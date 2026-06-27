#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{ProductAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::{OutputFormat, ProductListType};

async fn mount_one_product(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1]})))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "name": "TestProduct", "description": "A test product"}]
        })))
        .mount(mock)
        .await;
}

fn list_with(projection: ProjectionArgs) -> ProductAction {
    ProductAction::List {
        r#type: ProductListType::Accessible,
        projection,
    }
}

#[tokio::test]
async fn product_list_returns_products() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "TestProduct",
                "description": "A test product"
            }]
        })))
        .mount(&mock)
        .await;

    let action = ProductAction::List {
        r#type: ProductListType::Accessible,
        projection: ProjectionArgs::default(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["name"], "TestProduct");
}

#[tokio::test]
async fn product_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ProductAction::List {
        r#type: ProductListType::Accessible,
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
async fn product_list_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_product(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("name".into()),
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
    assert_eq!(parsed[0]["name"], "TestProduct");
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn product_list_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = list_with(ProjectionArgs {
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

#[tokio::test]
async fn product_list_table_fields_is_noop_with_warning() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_product(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("name".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(io.out_str().contains("TestProduct"));
    assert!(io
        .err_str()
        .contains("--fields/--exclude-fields only affect"));
}
