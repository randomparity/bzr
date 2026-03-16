use crate::cli::FieldAction;
use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &FieldAction,
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
        FieldAction::List { name } => {
            let values = client.get_field_values(name).await?;
            output::print_field_values(name, &values, format);
        }
    }
    Ok(())
}
