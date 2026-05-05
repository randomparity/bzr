use serde::Deserialize;

use super::encode_path;
use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::Classification;

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
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
