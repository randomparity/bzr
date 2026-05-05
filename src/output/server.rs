use colored::Colorize;
use serde::Serialize;

use super::formatting::print_formatted;
use crate::types::{OutputFormat, ServerInfoResponse};

/// Combined server information for display.
#[derive(Serialize)]
#[non_exhaustive]
struct ServerInfo<'a> {
    version: &'a str,
    extensions: &'a std::collections::HashMap<String, crate::types::ExtensionInfo>,
}

impl<'a> From<&'a ServerInfoResponse> for ServerInfo<'a> {
    fn from(info: &'a ServerInfoResponse) -> Self {
        Self {
            version: &info.version.version,
            extensions: &info.extensions.extensions,
        }
    }
}

#[expect(clippy::print_stdout)]
pub fn print_server_info(response: &ServerInfoResponse, format: OutputFormat) {
    let info = ServerInfo::from(response);
    print_formatted(&info, format, |info| {
        println!("{} {}", "Bugzilla version:".bold(), info.version);
        if info.extensions.is_empty() {
            println!("\nNo extensions installed.");
        } else {
            println!("\n{}:", "Extensions".bold());
            for (name, ext) in info.extensions {
                let ver = ext.version.as_deref().unwrap_or("unknown");
                println!("  {name} ({ver})");
            }
        }
    });
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
