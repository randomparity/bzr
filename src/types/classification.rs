use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Classification {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub products: Vec<ClassificationProduct>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClassificationProduct {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
