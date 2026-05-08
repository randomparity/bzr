use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::field_aliases::resolve_field_alias;
use crate::types::FieldValue;

#[derive(Deserialize)]
struct FieldBugResponse {
    fields: Vec<FieldEntry>,
}

#[derive(Deserialize)]
struct FieldEntry {
    values: Vec<FieldValue>,
}

impl BugzillaClient {
    /// Fetch legal values for a bug field.
    ///
    /// Returns `NotFound` when the server does not recognize the field name
    /// (empty `fields` array). An empty `Vec` means the field exists but has
    /// no legal values.
    pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
        let resolved = resolve_field_alias(field_name);
        let data: FieldBugResponse = self.get_json(&format!("field/bug/{resolved}")).await?;
        let field = data
            .fields
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "field",
                id: field_name.to_string(),
            })?;
        Ok(field.values)
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
