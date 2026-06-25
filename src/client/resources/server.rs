use crate::client::BugzillaClient;
use crate::error::Result;
use crate::types::server_info::{ServerExtensions, ServerInfoResponse, ServerVersion};

impl BugzillaClient {
    /// Fetch version and extensions from the server (two sequential requests).
    pub async fn server_info(&self) -> Result<ServerInfoResponse> {
        let version = self.server_version().await?;
        let extensions = self.server_extensions().await?;
        Ok(ServerInfoResponse {
            version,
            extensions,
        })
    }

    pub async fn server_version(&self) -> Result<ServerVersion> {
        self.get_json("version").await
    }

    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        self.get_json("extensions").await
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
