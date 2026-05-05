#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::test_client;

#[tokio::test]
async fn get_classification_returns_data() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{
                "id": 1,
                "name": "Unclassified",
                "description": "Default",
                "sort_key": 0,
                "products": [
                    {"id": 10, "name": "Widget", "description": "A widget"}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let cls = client.get_classification("Unclassified").await.unwrap();
    assert_eq!(cls.name, "Unclassified");
    assert_eq!(cls.products.len(), 1);
    assert_eq!(cls.products[0].name, "Widget");
}
