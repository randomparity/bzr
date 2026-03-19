use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProductListType {
    #[default]
    Accessible,
    Selectable,
    Enterable,
}

impl std::str::FromStr for ProductListType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "accessible" => Ok(Self::Accessible),
            "selectable" => Ok(Self::Selectable),
            "enterable" => Ok(Self::Enterable),
            other => Err(format!(
                "invalid product type '{other}': expected 'accessible', 'selectable', or 'enterable'"
            )),
        }
    }
}

impl ProductListType {
    pub fn as_endpoint(self) -> &'static str {
        match self {
            Self::Accessible => "product_accessible",
            Self::Selectable => "product_selectable",
            Self::Enterable => "product_enterable",
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
    pub fn as_char(self) -> char {
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
        write!(f, "{}", self.as_char())
    }
}

impl Serialize for FlagStatus {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_char(self.as_char())
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

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExtensionInfo {
    #[serde(default)]
    pub version: Option<String>,
}
