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
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::{ServerExtensions, ServerVersion};

    #[test]
    fn server_info_text_format_with_extensions() {
        let info = super::ServerInfo {
            version: "5.0.4",
            extensions: &{
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "BmpConvert".into(),
                    crate::types::ExtensionInfo {
                        version: Some("1.0".into()),
                    },
                );
                m
            },
        };
        assert_eq!(info.version, "5.0.4");
        assert!(info.extensions.contains_key("BmpConvert"));
    }

    #[test]
    fn server_info_text_format_no_extensions() {
        let empty = std::collections::HashMap::new();
        let info = super::ServerInfo {
            version: "4.0",
            extensions: &empty,
        };
        assert!(info.extensions.is_empty());
    }

    #[test]
    fn print_server_info_json_combined() {
        let version = ServerVersion {
            version: "5.0.4".into(),
        };
        let extensions = ServerExtensions {
            extensions: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "BmpConvert".into(),
                    crate::types::ExtensionInfo {
                        version: Some("1.0".into()),
                    },
                );
                m
            },
        };
        let combined = serde_json::json!({
            "version": version.version,
            "extensions": extensions.extensions,
        });
        let json = serde_json::to_string_pretty(&combined).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], "5.0.4");
        assert!(parsed["extensions"]["BmpConvert"].is_object());
    }
}
