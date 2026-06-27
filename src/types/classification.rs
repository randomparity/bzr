use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Classification {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_key: Option<u64>,
    #[serde(default)]
    pub products: Vec<ClassificationProduct>,
}

/// Serde JSON keys of [`Classification`], for `--fields` / `--exclude-fields`
/// validation on `classification list` and `classification view`.
pub const CLASSIFICATION_FIELDS: &[&str] = &["id", "name", "description", "sort_key", "products"];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClassificationProduct {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
