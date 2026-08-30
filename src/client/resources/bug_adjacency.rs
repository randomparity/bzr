use crate::client::BugzillaClient;
use crate::error::Result;
use crate::types::ApiMode;
use crate::types::{BugAdjacencyBug, BugAdjacencyError};

const BUG_ADJACENCY_FIELDS: &str = "id,summary,status,resolution,product,version,assigned_to,last_change_time,target_milestone,blocks,depends_on";

impl BugzillaClient {
    pub async fn get_bug_adjacency(
        &self,
        requested: &str,
    ) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
        match self.api_mode {
            ApiMode::Rest | ApiMode::Hybrid => self.get_bug_adjacency_rest(requested).await,
            ApiMode::XmlRpc => {
                self.strict_xmlrpc_client()
                    .get_bug_adjacency(requested)
                    .await
            }
        }
    }

    async fn get_bug_adjacency_rest(
        &self,
        requested: &str,
    ) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
        let request = self.strict_http.get(self.url("bug/")).query(&[
            ("ids", requested),
            ("include_fields", BUG_ADJACENCY_FIELDS),
            ("permissive", "1"),
        ]);
        let response = self.send_strict_once(request).await?;
        self.parse_strict_bug_adjacency_response(response, requested)
            .await
    }
}

#[cfg(test)]
#[path = "bug_adjacency_tests.rs"]
mod tests;
