use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;

#[expect(clippy::print_stdout)]
pub fn execute(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::SetServer {
            name,
            url,
            api_key,
            email,
        } => {
            let mut config = Config::load()?;
            config.servers.insert(
                name.clone(),
                ServerConfig {
                    url: url.clone(),
                    api_key: api_key.clone(),
                    email: email.clone(),
                    auth_method: None,
                },
            );
            if config.default_server.is_none() {
                config.default_server = Some(name.clone());
            }
            config.save()?;
            println!("Server '{name}' configured at {url}");
            if config.default_server.as_deref() == Some(name.as_str()) {
                println!("Set as default server.");
            }
        }
        ConfigAction::SetDefault { name } => {
            let mut config = Config::load()?;
            if !config.servers.contains_key(name) {
                return Err(crate::error::BzrError::config(format!(
                    "server '{name}' not found"
                )));
            }
            config.default_server = Some(name.clone());
            config.save()?;
            println!("Default server set to '{name}'");
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::path()?;
            println!("Config file: {}\n", path.display());
            if let Some(ref def) = config.default_server {
                println!("Default server: {def}");
            }
            if config.servers.is_empty() {
                println!("No servers configured.");
            } else {
                for (name, srv) in &config.servers {
                    let masked_key = if srv.api_key.len() > 8 {
                        format!("{}...", &srv.api_key[..8])
                    } else {
                        "***".into()
                    };
                    let auth_display = match &srv.auth_method {
                        Some(m) => m.to_string(),
                        None => "auto (not yet detected)".into(),
                    };
                    println!("\n[{name}]");
                    println!("  URL:     {}", srv.url);
                    if let Some(ref email) = srv.email {
                        println!("  Email:   {email}");
                    }
                    println!("  API Key: {masked_key}");
                    println!("  Auth:    {auth_display}");
                }
            }
        }
    }
    Ok(())
}
