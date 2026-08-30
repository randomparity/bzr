use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Component {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default, alias = "default_assigned_to")]
    pub default_assignee: Option<String>,
}

/// Serde JSON keys of [`Component`], for `--fields` / `--exclude-fields`
/// validation on `component list` and `component view`.
pub const COMPONENT_FIELDS: &[&str] =
    &["id", "name", "description", "is_active", "default_assignee"];

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateComponentParams {
    pub product: String,
    pub name: String,
    pub description: String,
    pub default_assignee: String,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateComponentParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_assignee: Option<String>,
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
