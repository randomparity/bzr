#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::test_client;
use crate::error::BzrError;
use crate::types::{CreateProductParams, ProductListType, UpdateProductParams};

#[test]
fn list_products_rejects_invalid_ids_with_stable_messages() {
    for (id, expected) in [
        (
            serde_json::json!(0),
            "expected a positive integer product ID",
        ),
        (
            serde_json::json!(-5),
            "expected a positive integer product ID",
        ),
        (
            serde_json::json!("not_a_number"),
            "expected a positive integer product ID",
        ),
        (
            serde_json::json!("-5"),
            "expected a positive integer product ID",
        ),
        (
            serde_json::json!(true),
            "invalid type: boolean `true`, expected a positive integer or decimal numeric string product ID",
        ),
    ] {
        let error = serde_json::from_value::<super::ProductAccessibleResponse>(
            serde_json::json!({"ids": [id]}),
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), expected);
    }
}

#[tokio::test]
async fn list_products_returns_products() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .and(query_param("ids", "1"))
        .and(query_param("ids", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [
                {"id": 1, "name": "Widget", "description": "A widget", "is_active": true, "components": [], "versions": [], "milestones": []},
                {"id": 2, "name": "Gadget", "description": "A gadget", "is_active": true, "components": [], "versions": [], "milestones": []}
            ]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let products = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap();
    assert_eq!(products.len(), 2);
    assert_eq!(products[0].name.as_deref(), Some("Widget"));
    assert_eq!(products[1].name.as_deref(), Some("Gadget"));
}

#[tokio::test]
async fn list_products_empty() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let products = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap();
    assert!(products.is_empty());
}

#[tokio::test]
async fn list_products_accepts_string_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": ["1", "2"]})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .and(query_param("ids", "1"))
        .and(query_param("ids", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [
                {"id": 1, "name": "Widget", "description": "A widget", "is_active": true, "components": [], "versions": [], "milestones": []},
                {"id": 2, "name": "Gadget", "description": "A gadget", "is_active": true, "components": [], "versions": [], "milestones": []}
            ]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let products = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap();
    assert_eq!(products.len(), 2);
    assert_eq!(products[0].name.as_deref(), Some("Widget"));
    assert_eq!(products[1].name.as_deref(), Some("Gadget"));
}

#[tokio::test]
async fn list_products_accepts_mixed_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, "2"]})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .and(query_param("ids", "1"))
        .and(query_param("ids", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [
                {"id": 1, "name": "Widget", "description": "A widget", "is_active": true, "components": [], "versions": [], "milestones": []},
                {"id": 2, "name": "Gadget", "description": "A gadget", "is_active": true, "components": [], "versions": [], "milestones": []}
            ]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let products = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap();
    assert_eq!(products.len(), 2);
}

#[tokio::test]
async fn list_products_rejects_malformed_string_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": ["not_a_number"]})),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Deserialize(_)),
        "expected deserialize error, got: {err:?}"
    );
}

#[tokio::test]
async fn list_products_rejects_negative_or_zero_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [0]})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Deserialize(_)),
        "expected deserialize error, got: {err:?}"
    );
}

#[tokio::test]
async fn list_products_rejects_string_zero_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": ["0"]})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Deserialize(_)),
        "expected deserialize error, got: {err:?}"
    );
}

#[tokio::test]
async fn list_products_rejects_negative_string_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": ["-5"]})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Deserialize(_)),
        "expected deserialize error, got: {err:?}"
    );
}

#[tokio::test]
async fn list_products_rejects_negative_number_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [-5]})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .list_products_by_type(ProductListType::Accessible)
        .await
        .unwrap_err();
    assert!(
        matches!(err, BzrError::Deserialize(_)),
        "expected deserialize error, got: {err:?}"
    );
}

#[tokio::test]
async fn get_product_by_name() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "Widget",
                "description": "A widget",
                "is_active": true,
                "components": [{"id": 10, "name": "Backend", "description": "", "is_active": true}],
                "versions": [{"id": 20, "name": "1.0", "sort_key": 0, "is_active": true}],
                "milestones": [{"id": 30, "name": "M1", "sort_key": 0, "is_active": true}]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let product = client.get_product("Widget").await.unwrap();
    assert_eq!(product.name.as_deref(), Some("Widget"));
    assert_eq!(product.components.len(), 1);
    assert_eq!(product.components[0].name.as_deref(), Some("Backend"));
    assert_eq!(product.versions.len(), 1);
    assert_eq!(product.milestones.len(), 1);
}

#[tokio::test]
async fn get_product_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"products": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client.get_product("NoSuch").await.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn create_product_returns_id() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .and(body_json(serde_json::json!({
            "name": "NewProduct",
            "description": "A new product",
            "version": "1.0",
            "is_open": true,
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 42})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = CreateProductParams {
        name: "NewProduct".into(),
        description: "A new product".into(),
        version: "1.0".into(),
        is_open: true,
    };
    let id = client.create_product(&params).await.unwrap();
    assert_eq!(id, 42);
}

#[tokio::test]
async fn create_product_forbidden() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized to create products."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = CreateProductParams {
        name: "X".into(),
        description: "X".into(),
        version: "1.0".into(),
        is_open: true,
    };
    let err = client.create_product(&params).await.unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}

#[tokio::test]
async fn update_product_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/product/Widget"))
        .and(body_json(serde_json::json!({
            "description": "Updated desc",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = UpdateProductParams {
        description: Some("Updated desc".into()),
        ..Default::default()
    };
    client.update_product("Widget", &params).await.unwrap();
}

#[tokio::test]
async fn update_product_forbidden() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/product/Widget"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = UpdateProductParams::default();
    let err = client.update_product("Widget", &params).await.unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}
