use crate::client::BugzillaClient;
use crate::error::Result;
use crate::types::component::{CreateComponentParams, UpdateComponentParams};

impl BugzillaClient {
    pub async fn create_component(&self, params: &CreateComponentParams) -> Result<u64> {
        self.post_json_id("component", params).await
    }

    pub async fn update_component(&self, params: UpdateComponentParams<'_>) -> Result<()> {
        self.xmlrpc_client().update_component(params).await
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
