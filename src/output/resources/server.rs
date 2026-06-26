use std::io::Write;

use colored::Colorize;
use serde::Serialize;

use crate::output::formatting::{write_formatted, yes_no};
use crate::types::capabilities::ServerCapabilities;
use crate::types::output::OutputFormat;
use crate::types::server_info::{ExtensionInfo, ServerInfoResponse};

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

pub fn write_server_capabilities<W: Write + ?Sized>(
    caps: &ServerCapabilities,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(caps, format, out, |caps, out| {
        write_capabilities_table(caps, out);
    });
}

fn write_capabilities_table<W: Write + ?Sized>(caps: &ServerCapabilities, out: &mut W) {
    let _ = writeln!(out, "{} {}", "Bugzilla version:".bold(), caps.version);
    let _ = writeln!(out, "{} {}", "API modes:".bold(), caps.api_modes.join(", "));
    let _ = writeln!(
        out,
        "{} {}",
        "Auth modes:".bold(),
        caps.auth_modes.join(", ")
    );
    let size = caps
        .max_attachment_size
        .map_or_else(|| "unknown".to_string(), |bytes| format!("{bytes} bytes"));
    let _ = writeln!(out, "{} {size}", "Max attachment size:".bold());

    let _ = writeln!(out, "\n{}", "Supports".bold());
    let _ = writeln!(out, "  comments       {}", yes_no(caps.supports_comments));
    let _ = writeln!(
        out,
        "  attachments    {}",
        yes_no(caps.supports_attachments)
    );
    let _ = writeln!(out, "  history        {}", yes_no(caps.supports_history));
    let _ = writeln!(
        out,
        "  flag requests  {}",
        yes_no(caps.supports_flag_requests)
    );

    let _ = writeln!(out, "\n{}", "Status transitions".bold());
    if caps.status_transitions.is_empty() {
        let _ = writeln!(out, "  (none reported)");
    } else {
        for transition in &caps.status_transitions {
            let _ = writeln!(
                out,
                "  {} → {}",
                transition.from,
                transition.can_change_to.join(", ")
            );
        }
    }

    let _ = writeln!(out, "\n{}", "Custom fields".bold());
    if caps.custom_fields.is_empty() {
        let _ = writeln!(out, "  (none)");
    } else {
        for field in &caps.custom_fields {
            let values = field.values.join(", ");
            let _ = writeln!(out, "  {} ({}): {values}", field.name, field.field_type);
        }
    }

    let flags = caps.flag_types.as_ref().map_or_else(
        || "undetermined".to_string(),
        |types| format!("{}", types.len()),
    );
    let _ = writeln!(out, "\n{} {flags}", "Flag types:".bold());
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
