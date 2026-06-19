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

#[tokio::test]
async fn list_classifications_enumerates_via_field_values() {
    let mock = MockServer::start().await;
    // Names come from the `classification` bug-field's legal values.
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/classification"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "values": [
                    {"name": "Acme", "sort_key": 5},
                    {"name": "Unclassified", "sort_key": 0}
                ]
            }]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Acme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{"id": 2, "name": "Acme", "description": "Acme group", "sort_key": 5, "products": []}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{"id": 1, "name": "Unclassified", "description": "Default", "sort_key": 0, "products": []}]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let list = client.list_classifications().await.unwrap();
    assert_eq!(list.len(), 2);
    // Sorted by sort_key: Unclassified (0) before Acme (5).
    assert_eq!(list[0].name, "Unclassified");
    assert_eq!(list[1].name, "Acme");
    assert_eq!(list[1].id, 2);
}
