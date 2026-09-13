use crate::cli::AuthAction;
use crate::client::{BugzillaClient, BugzillaClientConfig};
use crate::commands::runtime::invocation::{CommandCapabilities, CommandContext};
use crate::config::{Config, ServerConfig};
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;
use crate::tls::TlsConfig;

pub(crate) async fn execute(
    action: &AuthAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if ctx.inline_server().is_some() {
        return Err(BzrError::input(
            "auth commands require a named server so the login token can be stored".into(),
        ));
    }
    let config = Config::load_at(ctx.config_path_override())?;
    let (name, server) = config.resolve_server(ctx.server())?;
    match action {
        AuthAction::Login {
            email,
            password,
            restrict_login,
        } => {
            ensure_token_slot(server, name)?;
            let password = password.clone().map_or_else(read_password, Ok)?;
            let client = unauthenticated_client(server, name, ctx).await?;
            let token = client.login(email, &password, *restrict_login).await?;
            Config::update_locked_at(ctx.config_path_override(), |config| {
                let server = config.servers.get_mut(name).ok_or_else(|| {
                    BzrError::config(format!(
                        "server '{name}' was removed while login was running"
                    ))
                })?;
                ensure_token_slot(server, name)?;
                server.token = Some(token);
                Ok(())
            })?;
            write_result(
                &serde_json::json!({"server": name, "action": "logged-in"}),
                &format!("Logged in to server '{name}'."),
                ctx.format(),
                w.out,
            );
        }
        AuthAction::Logout => {
            let token = server.token.as_deref().ok_or_else(|| {
                BzrError::config(format!("server '{name}' has no saved login token"))
            })?;
            let client = unauthenticated_client(server, name, ctx).await?;
            client.logout(token).await?;
            Config::update_locked_at(ctx.config_path_override(), |config| {
                let server = config.servers.get_mut(name).ok_or_else(|| {
                    BzrError::config(format!(
                        "server '{name}' was removed while logout was running"
                    ))
                })?;
                server.token = None;
                Ok(())
            })?;
            write_result(
                &serde_json::json!({"server": name, "action": "logged-out"}),
                &format!("Logged out from server '{name}'."),
                ctx.format(),
                w.out,
            );
        }
    }
    Ok(())
}

fn ensure_token_slot(server: &ServerConfig, name: &str) -> Result<()> {
    if server.api_key.is_some() || server.api_key_env.is_some() || server.api_key_keyring.is_some()
    {
        return Err(BzrError::config(format!("server '{name}' already has an API-key credential source; remove it before using auth login")));
    }
    Ok(())
}

fn read_password() -> Result<String> {
    rpassword::prompt_password("Bugzilla password (input hidden): ").map_err(|error| {
        BzrError::Io(std::io::Error::other(format!(
            "failed to read Bugzilla password from stdin: {error}"
        )))
    })
}

async fn unauthenticated_client(
    server: &ServerConfig,
    name: &str,
    ctx: &CommandContext,
) -> Result<BugzillaClient> {
    let tls = TlsConfig {
        insecure: server.tls_insecure,
        ca_cert_path: server.tls_ca_cert.clone(),
        pin_sha256: server.tls_pin_sha256.clone(),
        pin_issuer_der: server.tls_pin_issuer_der.clone(),
        server_name: Some(name.to_owned()),
    };
    let mode = match ctx.api() {
        Some(mode) => mode,
        None => {
            crate::client::detect_server_settings_without_auth(
                &server.url,
                &tls,
                ctx.request_timeout(),
            )
            .await?
            .api_mode
        }
    };
    BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.url,
        credential: None,
        token: None,
        auth_method: None,
        api_mode: mode,
        email_hint: server.email.as_deref(),
        server_name: name,
        tls_config: &tls,
        request_timeout: ctx.request_timeout(),
        retry_max: ctx.retry_max(),
    })
}

pub(crate) fn capabilities(_: &AuthAction) -> CommandCapabilities {
    CommandCapabilities::anonymous()
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
