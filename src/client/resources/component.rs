use crate::client::BugzillaClient;
use crate::error::Result;
use crate::types::component::CreateComponentParams;

impl BugzillaClient {
    pub async fn create_component(&self, params: &CreateComponentParams) -> Result<u64> {
        self.post_json_id("component", params).await
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
