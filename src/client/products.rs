use serde::Deserialize;

use super::encode_path;
use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{CreateProductParams, Product, ProductListType, UpdateProductParams};

#[derive(Deserialize)]
struct ProductAccessibleResponse {
    ids: Vec<u64>,
}

#[derive(Deserialize)]
struct ProductResponse {
    products: Vec<Product>,
}

impl BugzillaClient {
    pub async fn list_products_by_type(
        &self,
        product_type: ProductListType,
    ) -> Result<Vec<Product>> {
        let endpoint = product_type.as_endpoint();
        let req = self.apply_auth(self.http.get(self.url(endpoint)));
        let resp = self.send(req).await?;
        let accessible: ProductAccessibleResponse = self.parse_json(resp).await?;

        if accessible.ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_products = Vec::new();
        for chunk in accessible.ids.chunks(50) {
            let id_params: Vec<(&str, String)> =
                chunk.iter().map(|id| ("ids", id.to_string())).collect();
            let req = self.apply_auth(self.http.get(self.url("product")).query(&id_params));
            let resp = self.send(req).await?;
            let data: ProductResponse = self.parse_json(resp).await?;
            all_products.extend(data.products);
        }
        Ok(all_products)
    }

    pub async fn create_product(&self, params: &CreateProductParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("product")).json(params));
        let resp = self.send(req).await?;
        let data: super::IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_product(&self, name: &str, updates: &UpdateProductParams) -> Result<()> {
        self.put_json(&format!("product/{}", encode_path(name)), updates)
            .await
    }

    /// Fetch a product by name. Note: components, versions, and milestones
    /// may require `include_fields` on some Bugzilla versions to be populated.
    pub async fn get_product(&self, name: &str) -> Result<Product> {
        let req = self.apply_auth(self.http.get(self.url("product")).query(&[("names", name)]));
        let resp = self.send(req).await?;
        let data: ProductResponse = self.parse_json(resp).await?;
        data.products
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "product",
                id: name.to_string(),
            })
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;
    use crate::types::{CreateProductParams, ProductListType, UpdateProductParams};

    #[tokio::test]
    async fn list_products_returns_products() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product_accessible"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})),
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
        assert_eq!(products[0].name, "Widget");
        assert_eq!(products[1].name, "Gadget");
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
        assert_eq!(product.name, "Widget");
        assert_eq!(product.components.len(), 1);
        assert_eq!(product.components[0].name, "Backend");
        assert_eq!(product.versions.len(), 1);
        assert_eq!(product.milestones.len(), 1);
    }

    #[tokio::test]
    async fn get_product_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"products": []})),
            )
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
}
