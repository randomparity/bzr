use serde::{Deserialize, Serialize};

use super::common::FlagUpdate;

#[derive(Debug, Serialize, Deserialize)]
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
    #[serde(default)]
    pub is_obsolete: bool,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub data: Option<String>,
}

pub struct UploadAttachmentParams {
    pub bug_id: u64,
    pub file_name: String,
    pub summary: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub flags: Vec<FlagUpdate>,
}

#[derive(Debug, Default, Serialize)]
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
