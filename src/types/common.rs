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
