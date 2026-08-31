use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;

use crate::client::encode_path;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::product::{CreateProductParams, Product, ProductListType, UpdateProductParams};

struct ProductIdVisitor;

impl Visitor<'_> for ProductIdVisitor {
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a positive integer or decimal numeric string product ID")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
        if value == 0 {
            return Err(E::custom("expected a positive integer product ID"));
        }
        Ok(value)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
        if value <= 0 {
            return Err(E::custom("expected a positive integer product ID"));
        }
        u64::try_from(value).map_err(|_| E::custom("expected a positive integer product ID"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
        let parsed: u64 = value
            .parse()
            .map_err(|_| E::custom("expected a positive integer product ID"))?;
        if parsed == 0 {
            return Err(E::custom("expected a positive integer product ID"));
        }
        Ok(parsed)
    }
}

fn deserialize_product_ids<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<u64>, D::Error> {
    struct ProductIdWrapper(u64);

    impl<'de> Deserialize<'de> for ProductIdWrapper {
        fn deserialize<D2: Deserializer<'de>>(
            deserializer: D2,
        ) -> std::result::Result<Self, D2::Error> {
            deserializer
                .deserialize_any(ProductIdVisitor)
                .map(ProductIdWrapper)
        }
    }

    let wrappers: Vec<ProductIdWrapper> = Vec::deserialize(deserializer)?;
    Ok(wrappers.into_iter().map(|w| w.0).collect())
}

#[derive(Deserialize)]
struct ProductAccessibleResponse {
    #[serde(deserialize_with = "deserialize_product_ids")]
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
        let endpoint = product_type.as_api_path();
        let accessible: ProductAccessibleResponse = self.get_json(endpoint).await?;

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
        self.post_json_id("product", params).await
    }

    pub async fn update_product(&self, name: &str, updates: &UpdateProductParams) -> Result<()> {
        self.put_json(&format!("product/{}", encode_path(name)), updates)
            .await
    }

    /// Fetch a product by name. Note: components, versions, and milestones
    /// may require `include_fields` on some Bugzilla versions to be populated.
    pub async fn get_product(&self, name: &str) -> Result<Product> {
        let data: ProductResponse = self.get_json_query("product", &[("names", name)]).await?;
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
#[path = "product_tests.rs"]
mod tests;
