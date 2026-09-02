use serde::{Deserialize, Deserializer};

use crate::client::{encode_path, BugzillaClient};
use crate::error::{BzrError, Result};
use crate::types::deserialization::u64_from_number_or_string;
use crate::types::{resolve_field_alias, FieldValue};

#[derive(Default)]
pub(super) struct UnsignedWire(pub(super) u64);

impl<'de> Deserialize<'de> for UnsignedWire {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64_from_number_or_string(
            deserializer,
            "a non-negative integer or decimal numeric string",
            "expected a non-negative integer",
        )
        .map(Self)
    }
}

#[derive(Deserialize)]
pub(super) struct FieldDefinition {
    pub(super) name: String,
    #[serde(rename = "type", default)]
    pub(super) field_type: UnsignedWire,
    #[serde(default)]
    pub(super) is_custom: bool,
    #[serde(default)]
    pub(super) values: Vec<FieldValue>,
}

#[derive(Deserialize)]
struct FieldBugResponse {
    fields: Vec<FieldDefinition>,
}

impl BugzillaClient {
    /// Fetch legal values for a bug field.
    ///
    /// Returns `NotFound` when the server does not recognize the field name
    /// (empty `fields` array). An empty `Vec` means the field exists but has
    /// no legal values.
    pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
        let resolved = resolve_field_alias(field_name);
        if matches!(resolved.as_ref(), "" | "." | "..") {
            return Err(BzrError::InputValidation {
                message: "field name must not be empty, '.', or '..'".to_string(),
                field: Some("field".to_string()),
                value: Some(field_name.to_string()),
            });
        }
        let data: FieldBugResponse = self
            .get_json(&format!("field/bug/{}", encode_path(resolved.as_ref())))
            .await?;
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

    /// Fetch all bug field definitions.
    pub(super) async fn all_bug_fields(&self) -> Result<Vec<FieldDefinition>> {
        Ok(self.get_json::<FieldBugResponse>("field/bug").await?.fields)
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
