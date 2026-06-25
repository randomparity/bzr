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

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
