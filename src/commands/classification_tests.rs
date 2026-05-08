#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ClassificationAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn classification_view_returns_data() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{
                "id": 1,
                "name": "Unclassified",
                "description": "Not yet classified",
                "products": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = ClassificationAction::View {
        name: "Unclassified".to_string(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "Unclassified");
    assert_eq!(parsed["description"], "Not yet classified");
}

#[tokio::test]
async fn classification_view_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Missing"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ClassificationAction::View {
        name: "Missing".to_string(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
