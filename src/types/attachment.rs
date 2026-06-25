use serde::{Deserialize, Deserializer, Serialize};

use super::flag::{Flag, FlagUpdate};

/// Deserialize an optional boolean that may arrive as an integer (0/1) from Bugzilla 5.0.
fn option_bool_from_int_or_bool<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Bool(b) => Ok(Some(b)),
        serde_json::Value::Number(n) => match n.as_u64() {
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            _ => Err(serde::de::Error::custom(format!(
                "expected bool or 0/1 integer, got {n}"
            ))),
        },
        other => Err(serde::de::Error::custom(format!(
            "expected bool or integer, got {other}"
        ))),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Attachment {
    pub id: u64,
    /// Parent bug when the server included it; `id` stays required because it
    /// is the primary key.
    #[serde(default)]
    pub bug_id: Option<u64>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
    pub is_obsolete: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
    pub is_private: Option<bool>,
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
    pub is_patch: Option<bool>,
    #[serde(default)]
    pub flags: Vec<Flag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct UploadAttachmentParams {
    #[serde(rename = "ids", serialize_with = "serialize_bug_id_as_array")]
    pub bug_id: u64,
    pub file_name: String,
    pub summary: String,
    pub content_type: String,
    #[serde(serialize_with = "serialize_data_as_base64")]
    pub data: Vec<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
    pub is_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub is_patch: bool,
}

// Serde serialize_with requires &T signature for the field type.
#[expect(clippy::trivially_copy_pass_by_ref)]
fn serialize_bug_id_as_array<S: serde::Serializer>(id: &u64, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(1))?;
    seq.serialize_element(id)?;
    seq.end()
}

fn serialize_data_as_base64<S: serde::Serializer>(data: &[u8], s: S) -> Result<S::Ok, S::Error> {
    use base64::Engine as _;
    s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(data))
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

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
