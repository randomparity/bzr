use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Header,
    QueryParam,
}

impl FromStr for AuthMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "header" => Ok(AuthMethod::Header),
            "query_param" => Ok(AuthMethod::QueryParam),
            _ => Err(format!(
                "invalid auth method '{s}': expected 'header' or 'query_param'"
            )),
        }
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthMethod::Header => write!(f, "header"),
            AuthMethod::QueryParam => write!(f, "query_param"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiMode {
    Rest,
    #[serde(rename = "xmlrpc")]
    XmlRpc,
    Hybrid,
}

impl fmt::Display for ApiMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiMode::Rest => write!(f, "rest"),
            ApiMode::XmlRpc => write!(f, "xmlrpc"),
            ApiMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl FromStr for ApiMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "rest" => Ok(ApiMode::Rest),
            "xmlrpc" => Ok(ApiMode::XmlRpc),
            "hybrid" => Ok(ApiMode::Hybrid),
            _ => Err(format!(
                "invalid API mode '{s}': expected 'rest', 'xmlrpc', or 'hybrid'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            _ => Err(format!(
                "invalid output format '{s}': expected 'table' or 'json'"
            )),
        }
    }
}

/// Represents the four valid flag status values in Bugzilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagStatus {
    /// `+` — flag granted
    Grant,
    /// `-` — flag denied
    Deny,
    /// `?` — flag requested
    Request,
    /// `X` — flag cleared/removed
    Clear,
}

impl FlagStatus {
    pub fn to_char(self) -> char {
        match self {
            FlagStatus::Grant => '+',
            FlagStatus::Deny => '-',
            FlagStatus::Request => '?',
            FlagStatus::Clear => 'X',
        }
    }
}

impl std::fmt::Display for FlagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

impl Serialize for FlagStatus {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_char(self.to_char())
    }
}

impl<'de> Deserialize<'de> for FlagStatus {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "+" => Ok(FlagStatus::Grant),
            "-" => Ok(FlagStatus::Deny),
            "?" => Ok(FlagStatus::Request),
            "X" => Ok(FlagStatus::Clear),
            other => Err(serde::de::Error::custom(format!(
                "invalid flag status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FlagUpdate {
    pub name: String,
    pub status: FlagStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestee: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServerVersion {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServerExtensions {
    pub extensions: HashMap<String, ExtensionInfo>,
}

/// Combined server version and extensions, returned by `BugzillaClient::server_info()`.
#[derive(Debug)]
#[non_exhaustive]
pub struct ServerInfoResponse {
    pub version: ServerVersion,
    pub extensions: ServerExtensions,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExtensionInfo {
    #[serde(default)]
    pub version: Option<String>,
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn flag_update_serializes_with_status_char() {
        let flag = FlagUpdate {
            name: "review".to_string(),
            status: FlagStatus::Grant,
            requestee: None,
        };
        let json = serde_json::to_value(&flag).unwrap();
        assert_eq!(json["name"], "review");
        assert_eq!(json["status"], "+");
        assert!(json.get("requestee").is_none());
    }

    #[test]
    fn flag_update_serializes_with_requestee() {
        let flag = FlagUpdate {
            name: "needinfo".to_string(),
            status: FlagStatus::Request,
            requestee: Some("user@example.com".to_string()),
        };
        let json = serde_json::to_value(&flag).unwrap();
        assert_eq!(json["name"], "needinfo");
        assert_eq!(json["status"], "?");
        assert_eq!(json["requestee"], "user@example.com");
    }

    #[test]
    fn flag_update_roundtrip() {
        let json = serde_json::json!({
            "name": "approval",
            "status": "-",
            "requestee": "admin@example.com"
        });
        let flag: FlagUpdate = serde_json::from_value(json).unwrap();
        assert_eq!(flag.name, "approval");
        assert_eq!(flag.status, FlagStatus::Deny);
        assert_eq!(flag.requestee.as_deref(), Some("admin@example.com"));
    }

    #[test]
    fn flag_status_all_variants_roundtrip() {
        for (ch, expected) in [
            ("+", FlagStatus::Grant),
            ("-", FlagStatus::Deny),
            ("?", FlagStatus::Request),
            ("X", FlagStatus::Clear),
        ] {
            let json = serde_json::json!(ch);
            let status: FlagStatus = serde_json::from_value(json).unwrap();
            assert_eq!(status, expected);

            let serialized = serde_json::to_value(status).unwrap();
            assert_eq!(serialized, ch);
        }
    }

    #[test]
    fn flag_status_invalid_deserialize() {
        let json = serde_json::json!("Z");
        let err = serde_json::from_value::<FlagStatus>(json).unwrap_err();
        assert!(err.to_string().contains("invalid flag status"));
    }

    #[test]
    fn output_format_from_str_valid() {
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Table
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    }

    #[test]
    fn output_format_from_str_invalid() {
        let err = "xml".parse::<OutputFormat>().unwrap_err();
        assert!(err.contains("invalid output format"));
    }

    #[test]
    fn output_format_default_is_table() {
        assert_eq!(OutputFormat::default(), OutputFormat::Table);
    }

    #[test]
    fn server_version_deserializes() {
        let json = serde_json::json!({"version": "5.0.4"});
        let ver: ServerVersion = serde_json::from_value(json).unwrap();
        assert_eq!(ver.version, "5.0.4");
    }

    #[test]
    fn server_extensions_deserializes() {
        let json = serde_json::json!({
            "extensions": {
                "MyExtension": {"version": "1.0"},
                "Bare": {}
            }
        });
        let ext: ServerExtensions = serde_json::from_value(json).unwrap();
        assert_eq!(ext.extensions.len(), 2);
        assert_eq!(
            ext.extensions["MyExtension"].version.as_deref(),
            Some("1.0")
        );
        assert!(ext.extensions["Bare"].version.is_none());
    }

    #[test]
    fn auth_method_from_str() {
        assert_eq!("header".parse::<AuthMethod>().unwrap(), AuthMethod::Header);
        assert_eq!(
            "query_param".parse::<AuthMethod>().unwrap(),
            AuthMethod::QueryParam
        );
        assert!("bogus".parse::<AuthMethod>().is_err());
    }

    #[test]
    fn api_mode_from_str() {
        assert_eq!("rest".parse::<ApiMode>().unwrap(), ApiMode::Rest);
        assert_eq!("xmlrpc".parse::<ApiMode>().unwrap(), ApiMode::XmlRpc);
        assert_eq!("hybrid".parse::<ApiMode>().unwrap(), ApiMode::Hybrid);
        assert!("grpc".parse::<ApiMode>().is_err());
    }
}
