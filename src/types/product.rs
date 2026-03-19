use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub versions: Vec<Version>,
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub default_assignee: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Milestone {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct CreateProductParams {
    pub name: String,
    pub description: String,
    pub version: String,
    pub is_open: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateProductParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_open: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateComponentParams {
    pub product: String,
    pub name: String,
    pub description: String,
    pub default_assignee: String,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateComponentParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_assignee: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
pub struct ClassificationProduct {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}
