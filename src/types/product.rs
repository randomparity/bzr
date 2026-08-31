use serde::{Deserialize, Serialize};

use crate::types::component::Component;

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Product {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub versions: Vec<Version>,
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

/// Serde JSON keys of [`Product`], for `--fields` / `--exclude-fields`
/// validation on `product list` and `product view`.
pub const PRODUCT_FIELDS: &[&str] = &[
    "id",
    "name",
    "description",
    "is_active",
    "components",
    "versions",
    "milestones",
];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Version {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::types::sort_key::deserialize_optional",
        serialize_with = "crate::types::sort_key::serialize_optional"
    )]
    pub sort_key: Option<i128>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Milestone {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::types::sort_key::deserialize_optional",
        serialize_with = "crate::types::sort_key::serialize_optional"
    )]
    pub sort_key: Option<i128>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateProductParams {
    pub name: String,
    pub description: String,
    pub version: String,
    pub is_open: bool,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateProductParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_open: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProductListType {
    #[default]
    Accessible,
    Selectable,
    Enterable,
}

impl std::str::FromStr for ProductListType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "accessible" => Ok(Self::Accessible),
            "selectable" => Ok(Self::Selectable),
            "enterable" => Ok(Self::Enterable),
            other => Err(format!(
                "invalid product type '{other}': expected 'accessible', 'selectable', or 'enterable'"
            )),
        }
    }
}

impl ProductListType {
    pub fn as_api_path(self) -> &'static str {
        match self {
            Self::Accessible => "product_accessible",
            Self::Selectable => "product_selectable",
            Self::Enterable => "product_enterable",
        }
    }
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
