#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::test_client;

#[tokio::test]
async fn server_version_returns_version() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let ver = client.server_version().await.unwrap();
    assert_eq!(ver.version, "5.0.4");
}

#[tokio::test]
async fn server_extensions_returns_map() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "extensions": {
                "BmpConvert": {"version": "1.0"},
                "InlineHistory": {"version": "2.1"}
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let ext = client.server_extensions().await.unwrap();
    assert_eq!(ext.extensions.len(), 2);
    assert!(ext.extensions.contains_key("BmpConvert"));
}
