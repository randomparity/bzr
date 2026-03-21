use crate::cli::ProductAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateProductParams, UpdateProductParams};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        ProductAction::List { r#type } => {
            let products = client.list_products_by_type(*r#type).await?;
            output::print_products(&products, format);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            output::print_product_detail(&product, format);
        }
        ProductAction::Create {
            name,
            description,
            version,
            is_open,
        } => {
            let params = CreateProductParams {
                name: name.clone(),
                description: description.clone(),
                version: version.clone(),
                is_open: *is_open,
            };
            let id = client.create_product(&params).await?;
            output::print_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Product),
                &format!("Created product #{id} '{name}'"),
                format,
            );
        }
        ProductAction::Update {
            name,
            description,
            default_milestone,
            is_open,
        } => {
            let params = UpdateProductParams {
                description: description.clone(),
                default_milestone: default_milestone.clone(),
                is_open: *is_open,
            };
            client.update_product(name, &params).await?;
            output::print_result(
                &ActionResult::updated_named(name.as_str(), ResourceKind::Product),
                &format!("Updated product '{name}'"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::setup_test_env;
    use crate::cli::ProductAction;
    use crate::types::{OutputFormat, ProductListType};

    #[tokio::test]
    async fn product_list_returns_products() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/product_accessible"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})),
            )
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
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn product_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/product_accessible"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = ProductAction::List {
            r#type: ProductListType::Accessible,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
    }
}
