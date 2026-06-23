use std::io::Write;

use serde::Serialize;

use crate::config::{CredentialSource, CredentialSourceKind, KeyringAccount};
use crate::output::formatting::{mask_api_key, write_field, write_formatted, write_optional_field};
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
            Ok(Some(CredentialSource::Inline(api_key))) => {
                (mask_api_key(api_key), CredentialSourceKind::Inline.as_str())
            }
            Ok(Some(CredentialSource::EnvVar(var_name))) => {
                (var_name.to_string(), CredentialSourceKind::Env.as_str())
            }
            Ok(Some(CredentialSource::Keyring { service, account })) => {
                let display = match account {
                    KeyringAccount::Explicit(account) => format!("{service}/{account}"),
                    KeyringAccount::ServerDefault => format!("{service}/<server-name>"),
                };
                (display, CredentialSourceKind::Keyring.as_str())
            }
            Ok(None) => ("none".to_string(), "none"),
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

fn write_api_key(out: &mut (impl Write + ?Sized), s: &ServerDisplayInfo) {
    let label = match s.api_key_source.as_str() {
        "env" => "API Key Env",
        "keyring" => "Keyring",
        _ => "API Key",
    };
    write_field(out, label, &s.api_key);
}

fn write_server(out: &mut (impl Write + ?Sized), name: &str, s: &ServerDisplayInfo) {
    let _ = writeln!(out, "\n[{name}]");
    write_field(out, "URL", &s.url);
    write_optional_field(out, "Email", s.email.as_deref());
    write_api_key(out, s);
    write_field(out, "API Key Source", &s.api_key_source);
    write_field(out, "Auth", &auth_display(s.auth_method.as_ref()));
    if s.tls_insecure {
        write_field(out, "TLS", "insecure (certificate verification disabled)");
    }
    if let Some(ca) = &s.tls_ca_cert {
        write_field(out, "TLS CA Cert", ca);
    }
    if let Some(pin) = &s.tls_pin {
        write_field(out, "TLS Pin", pin);
    }
}

pub fn write_config<W: Write + ?Sized>(view: &ConfigView, format: OutputFormat, out: &mut W) {
    write_formatted(view, format, out, |v, out| {
        let _ = writeln!(out, "Config file: {}\n", v.config_file);
        if let Some(ref def) = v.default_server {
            let _ = writeln!(out, "Default server: {def}");
        }
        if v.servers.is_empty() {
            let _ = writeln!(out, "No servers configured.");
            return;
        }
        for (name, s) in &v.servers {
            write_server(out, name, s);
        }
    });
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
