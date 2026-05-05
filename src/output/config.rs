use std::io::{self, Write};

use serde::Serialize;

use super::formatting::{mask_api_key, print_field, print_formatted, print_optional_field};
use crate::config::{CredentialSource, CredentialSourceKind};
use crate::types::{AuthMethod, OutputFormat};

fn auth_display(auth_method: Option<&AuthMethod>) -> String {
    auth_method.map_or_else(
        || "auto (not yet detected)".to_string(),
        ToString::to_string,
    )
}

#[derive(Serialize)]
#[non_exhaustive]
pub struct ServerDisplayInfo {
    url: String,
    email: Option<String>,
    api_key: String,
    api_key_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<AuthMethod>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tls_insecure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls_ca_cert: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls_pin: Option<String>,
}

impl ServerDisplayInfo {
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        let (api_key, api_key_source) = match srv.credential_source() {
            Ok(CredentialSource::Inline(api_key)) => {
                (mask_api_key(api_key), CredentialSourceKind::Inline.as_str())
            }
            Ok(CredentialSource::EnvVar(var_name)) => {
                (var_name.to_string(), CredentialSourceKind::Env.as_str())
            }
            Ok(CredentialSource::Keyring { service, account }) => {
                let display = if account.is_empty() {
                    format!("{service}/<server-name>")
                } else {
                    format!("{service}/{account}")
                };
                (display, CredentialSourceKind::Keyring.as_str())
            }
            Err(_) => ("[invalid config]".to_string(), "invalid"),
        };
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key,
            api_key_source: api_key_source.to_string(),
            auth_method: srv.auth_method,
            tls_insecure: srv.tls_insecure,
            tls_ca_cert: srv.tls_ca_cert.as_ref().map(|p| p.display().to_string()),
            tls_pin: srv.tls_pin_sha256.as_ref().map(|pin| {
                if let Some(issuer) = &srv.tls_pin_issuer {
                    format!("{pin} ({issuer})")
                } else {
                    pin.clone()
                }
            }),
        }
    }
}

#[derive(Serialize)]
#[non_exhaustive]
pub struct ConfigView {
    pub config_file: String,
    pub default_server: Option<String>,
    pub servers: std::collections::BTreeMap<String, ServerDisplayInfo>,
}

impl ConfigView {
    pub fn from_config(config: &crate::config::Config, path: &std::path::Path) -> Self {
        let servers = config
            .servers
            .iter()
            .map(|(name, srv)| (name.clone(), ServerDisplayInfo::from_config(srv)))
            .collect();
        Self {
            config_file: path.to_string_lossy().into_owned(),
            default_server: config.default_server.clone(),
            servers,
        }
    }
}

fn print_api_key(s: &ServerDisplayInfo) {
    let label = match s.api_key_source.as_str() {
        "env" => "API Key Env",
        "keyring" => "Keyring",
        _ => "API Key",
    };
    print_field(label, &s.api_key);
}

fn print_server(name: &str, s: &ServerDisplayInfo) {
    writeln!(io::stdout(), "\n[{name}]").expect("write to output");
    print_field("URL", &s.url);
    print_optional_field("Email", s.email.as_deref());
    print_api_key(s);
    print_field("API Key Source", &s.api_key_source);
    print_field("Auth", &auth_display(s.auth_method.as_ref()));
    if s.tls_insecure {
        print_field("TLS", "insecure (certificate verification disabled)");
    }
    if let Some(ca) = &s.tls_ca_cert {
        print_field("TLS CA Cert", ca);
    }
    if let Some(pin) = &s.tls_pin {
        print_field("TLS Pin", pin);
    }
}

pub fn print_config(view: &ConfigView, format: OutputFormat) {
    print_formatted(view, format, |v| {
        let mut out = io::stdout();
        writeln!(out, "Config file: {}\n", v.config_file).expect("write to output");
        if let Some(ref def) = v.default_server {
            writeln!(out, "Default server: {def}").expect("write to output");
        }
        if v.servers.is_empty() {
            writeln!(out, "No servers configured.").expect("write to output");
            return;
        }
        for (name, s) in &v.servers {
            print_server(name, s);
        }
    });
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
