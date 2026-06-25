use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
#[path = "server_info_tests.rs"]
mod tests;
