use std::borrow::Cow;

use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::FieldValue;

/// Known field name aliases mapped to their Bugzilla API internal names.
/// Sorted alphabetically by alias.
///
/// These aliases cannot shadow real Bugzilla field names because Bugzilla
/// requires custom fields to use the `cf_` prefix (e.g. `cf_status`), and
/// the built-in fields have fixed names (e.g. `bug_status`, `priority`).
/// No real field can have a bare name like `status` or `severity`, so eager
/// resolution is always safe.
pub(crate) const FIELD_ALIASES: &[(&str, &str)] = &[
    ("file_loc", "bug_file_loc"),
    ("group", "bug_group"),
    ("id", "bug_id"),
    ("severity", "bug_severity"),
    ("status", "bug_status"),
    ("type", "bug_type"),
];

#[derive(Deserialize)]
struct FieldBugResponse {
    fields: Vec<FieldEntry>,
}

#[derive(Deserialize)]
struct FieldEntry {
    values: Vec<FieldValue>,
}

fn resolve_field_alias(name: &str) -> Cow<'_, str> {
    let lower = name.to_ascii_lowercase();
    for &(alias, api_name) in FIELD_ALIASES {
        if lower == alias {
            return Cow::Borrowed(api_name);
        }
    }
    // Unknown fields pass through unchanged; only known aliases are normalized.
    Cow::Borrowed(name)
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
