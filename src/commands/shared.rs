use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::types::ApiMode;

/// Like [`connect_client_with_email`], but discards the server email.
/// Used by all commands that do not need the Bugzilla 5.0 `valid_login` fallback.
pub async fn connect_client(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let (client, _email) = connect_client_with_email(server, api_override).await?;
    Ok(client)
}

/// Connect to a Bugzilla server, also returning the server's configured email
/// (if any). The email is needed by whoami for Bugzilla 5.0 fallback.
pub async fn connect_client_with_email(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<(BugzillaClient, Option<String>)> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let (server_name, url, api_key, email) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
        srv.email.clone(),
    );
    let (auth, detected_mode) =
        crate::client::auth::detect_and_persist_server_settings(&mut config, &server_name).await?;
    let api_mode = api_override.unwrap_or(detected_mode);
    let client = BugzillaClient::new(&url, &api_key, auth, api_mode)?;
    Ok((client, email))
}
