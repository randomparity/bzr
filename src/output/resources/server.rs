use std::io::Write;

use colored::Colorize;
use serde::Serialize;

use crate::output::formatting::write_formatted;
use crate::types::common::{ExtensionInfo, OutputFormat, ServerInfoResponse};

/// Combined server information for display.
#[derive(Serialize)]
#[non_exhaustive]
struct ServerInfo<'a> {
    version: &'a str,
    extensions: &'a std::collections::HashMap<String, ExtensionInfo>,
}

impl<'a> From<&'a ServerInfoResponse> for ServerInfo<'a> {
    fn from(info: &'a ServerInfoResponse) -> Self {
        Self {
            version: &info.version.version,
            extensions: &info.extensions.extensions,
        }
    }
}

pub fn write_server_info<W: Write + ?Sized>(
    response: &ServerInfoResponse,
    format: OutputFormat,
    out: &mut W,
) {
    let info = ServerInfo::from(response);
    write_formatted(&info, format, out, |info, out| {
        let _ = writeln!(out, "{} {}", "Bugzilla version:".bold(), info.version);
        if info.extensions.is_empty() {
            let _ = writeln!(out, "\nNo extensions installed.");
        } else {
            let _ = writeln!(out, "\n{}:", "Extensions".bold());
            for (name, ext) in info.extensions {
                let ver = ext.version.as_deref().unwrap_or("unknown");
                let _ = writeln!(out, "  {name} ({ver})");
            }
        }
    });
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
