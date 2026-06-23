#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ProductAction;
use crate::test_helpers::setup_test_env;
use crate::types::{OutputFormat, ProductListType};

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
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    };
    let result = crate::commands::product::execute(
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
