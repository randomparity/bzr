use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GroupInfo {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub membership: Vec<GroupMember>,
}

/// Serde JSON keys of [`GroupInfo`], for `--fields` / `--exclude-fields`
/// validation on `group view`.
pub const GROUP_INFO_FIELDS: &[&str] = &["id", "name", "description", "is_active", "membership"];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GroupMember {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateGroupParams {
    pub name: String,
    pub description: String,
    pub is_active: bool,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateGroupParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
