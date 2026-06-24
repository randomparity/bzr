#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::test_client;

#[tokio::test]
async fn get_json_value_returns_parsed_value_without_typed_check() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/anything"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "arbitrary_key": "arbitrary_value",
            "nested": {"inner": 42}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let value = client.get_json_value("anything").await.unwrap();
    assert_eq!(value["arbitrary_key"], "arbitrary_value");
    assert_eq!(value["nested"]["inner"], 42);
}

#[tokio::test]
async fn get_json_value_runs_check_bugzilla_200_error() {
    // A 200 response with `error: true` and no data fields must still
    // produce a BzrError::Api — get_json_value should run the same
    // 200-error check that get_json does.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/anything"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 301,
            "message": "denied"
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("anything").await.unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::Api { code: 301, .. }),
        "expected Api error, got: {err}"
    );
}
