use serde::{Deserialize, Deserializer, Serialize};

use super::common::FlagUpdate;

/// Deserialize a boolean that may arrive as an integer (0/1) from Bugzilla 5.0.
fn bool_from_int_or_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::Number(n) => Ok(n.as_u64() != Some(0)),
        _ => Ok(false),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Attachment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default, deserialize_with = "bool_from_int_or_bool")]
    pub is_obsolete: bool,
    #[serde(default, deserialize_with = "bool_from_int_or_bool")]
    pub is_private: bool,
    #[serde(default)]
    pub data: Option<String>,
}

#[non_exhaustive]
pub struct UploadAttachmentParams {
    pub bug_id: u64,
    pub file_name: String,
    pub summary: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub flags: Vec<FlagUpdate>,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateAttachmentParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_obsolete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_patch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
}
