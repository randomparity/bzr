use std::collections::BTreeMap;

use crate::error::Result;
use crate::types::component::UpdateComponentParams;
use crate::xmlrpc::protocol::{Value, XmlRpcClient};

impl XmlRpcClient {
    pub async fn update_component(&self, params: UpdateComponentParams<'_>) -> Result<()> {
        let mut name = BTreeMap::new();
        name.insert("product".into(), Value::from(params.product));
        name.insert("component".into(), Value::from(params.component));

        let mut updates = BTreeMap::new();
        if let Some(description) = params.description {
            updates.insert("description".into(), Value::from(description));
        }
        if let Some(default_assignee) = params.default_assignee {
            updates.insert("default_assignee".into(), Value::from(default_assignee));
        }
        if let Some(is_active) = params.is_active {
            updates.insert("is_active".into(), Value::from(is_active));
        }

        let mut request = BTreeMap::new();
        request.insert("names".into(), Value::Array(vec![Value::Struct(name)]));
        request.insert("updates".into(), Value::Struct(updates));
        self.call("Component.update", request).await?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
