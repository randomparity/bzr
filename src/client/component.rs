use super::BugzillaClient;
use crate::error::Result;
use crate::types::component::{CreateComponentParams, UpdateComponentParams};

impl BugzillaClient {
    pub async fn create_component(&self, params: &CreateComponentParams) -> Result<u64> {
        self.post_json_id("component", params).await
    }

    /// Updates a component by numeric ID.
    ///
    /// Unlike peer update methods (`update_product`, `update_group`) which accept string names,
    /// the Bugzilla REST API's component endpoint only accepts numeric IDs.
    pub async fn update_component(&self, id: u64, updates: &UpdateComponentParams) -> Result<()> {
        self.put_json(&format!("component/{id}"), updates).await
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
