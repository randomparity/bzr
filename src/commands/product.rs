use crate::cli::ProductAction;
use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.active_server_named(server)?;
    let (server_name, url, api_key) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
    );
    let auth = crate::auth::resolve_auth_method(&mut config, &server_name).await?;
    let client = BugzillaClient::new(&url, &api_key, auth)?;

    match action {
        ProductAction::List => {
            let products = client.list_products().await?;
            output::print_products(&products, format);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            output::print_product_detail(&product, format);
        }
    }
    Ok(())
}
