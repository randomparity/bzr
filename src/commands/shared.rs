use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;

pub async fn build_client(server: Option<&str>) -> Result<BugzillaClient> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.active_server_named(server)?;
    let (server_name, url, api_key) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
    );
    let auth = crate::auth::resolve_auth_method(&mut config, &server_name).await?;
    BugzillaClient::new(&url, &api_key, auth)
}
