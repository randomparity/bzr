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
            "query_param" | "query-param" => Ok(AuthMethod::QueryParam),
            _ => Err(format!(
                "invalid auth method '{s}': expected 'header', 'query_param', or 'query-param'"
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
    /// Newline-delimited JSON: array outputs emit one compact value per line;
    /// single objects emit one compact line. Streaming-friendly for agents.
    Ndjson,
}

impl OutputFormat {
    /// Whether this format is a machine-readable JSON family (`json` or
    /// `ndjson`) as opposed to the human `table`. The two JSON families share
    /// every routing decision — serialization, error objects, stderr-only
    /// notes — and differ only in how the JSON is laid out, so call sites that
    /// branch "machine vs human" test this rather than spelling out both arms.
    #[must_use]
    pub fn is_json_family(self) -> bool {
        match self {
            OutputFormat::Json | OutputFormat::Ndjson => true,
            OutputFormat::Table => false,
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "ndjson" => Ok(OutputFormat::Ndjson),
            _ => Err(format!(
                "invalid output format '{s}': expected 'table', 'json', or 'ndjson'"
            )),
        }
    }
}

/// Sort direction for the `--order` flag on listing commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    /// The Bugzilla `order` keyword for this direction.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

impl std::str::FromStr for SortDirection {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "asc" | "ascending" => Ok(SortDirection::Asc),
            "desc" | "descending" => Ok(SortDirection::Desc),
            _ => Err(format!("invalid order '{s}': expected 'asc' or 'desc'")),
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

/// A flag as returned on a bug or attachment view response.
///
/// Distinct from the write-side [`FlagUpdate`]: `status` is the raw server
/// token (`"+"`, `"-"`, `"?"`) kept as a plain `String` so an unexpected value
/// cannot make a view fail to deserialize, and `setter` (who set the flag) is
/// surfaced. Every field defaults so a server that omits one still parses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Flag {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requestee: Option<String>,
}

impl Flag {
    /// Concise one-line rendering for table/detail output, symmetric with the
    /// `name?(user@example.com)` flag-input syntax: `name` + status token, with
    /// the requestee in parentheses when present (e.g. `review?`, `review+`,
    /// `review?(bob@example.com)`).
    pub fn render_inline(&self) -> String {
        match &self.requestee {
            Some(requestee) => format!("{}{}({requestee})", self.name, self.status),
            None => format!("{}{}", self.name, self.status),
        }
    }
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
#[path = "common_tests.rs"]
mod tests;
