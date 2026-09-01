use serde::{Deserialize, Deserializer, Serialize};

use crate::types::deserialization::{option_bool_from_int_or_bool, u64_from_number_or_string};

fn deserialize_group_or_member_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    u64_from_number_or_string(
        deserializer,
        "an unsigned integer or decimal numeric string group/member ID",
        "expected an unsigned integer group/member ID",
    )
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GroupInfo {
    #[serde(deserialize_with = "deserialize_group_or_member_id")]
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
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
    #[serde(deserialize_with = "deserialize_group_or_member_id")]
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
