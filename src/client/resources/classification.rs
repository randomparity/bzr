use serde::Deserialize;

use crate::client::encode_path;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::classification::Classification;

#[derive(Deserialize)]
struct ClassificationResponse {
    classifications: Vec<Classification>,
}

impl BugzillaClient {
    pub async fn get_classification(&self, name: &str) -> Result<Classification> {
        let data: ClassificationResponse = self
            .get_json(&format!("classification/{}", encode_path(name)))
            .await?;
        data.classifications
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "classification",
                id: name.to_string(),
            })
    }

    /// Enumerate the server's classifications.
    ///
    /// Bugzilla has no bulk "list classifications" REST endpoint, so the
    /// names are read from the `classification` bug field's legal values and
    /// each is then fetched for its full detail (id, description, products).
    /// Results are ordered by the field's `sort_key`, then name. On servers
    /// with classifications disabled, an unprivileged request can fail with API
    /// error 900 before the field exposes only `Unclassified`.
    pub async fn list_classifications(&self) -> Result<Vec<Classification>> {
        let values = self.get_field_values("classification").await?;
        let mut classifications = Vec::with_capacity(values.len());
        for value in values {
            let Some(name) = value.name.as_deref().filter(|name| !name.is_empty()) else {
                continue;
            };
            classifications.push(self.get_classification(name).await?);
        }
        classifications.sort_by(|a, b| {
            a.sort_key
                .cmp(&b.sort_key)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(classifications)
    }
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
