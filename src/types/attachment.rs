use serde::{Deserialize, Deserializer, Serialize};

use super::common::FlagUpdate;

/// Deserialize a boolean that may arrive as an integer (0/1) from Bugzilla 5.0.
fn bool_from_int_or_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::Number(n) => Ok(n.as_u64() != Some(0)),
        other => Err(serde::de::Error::custom(format!(
            "expected bool or integer, got {other}"
        ))),
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
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bool_from_int_or_bool_deserializes_true() {
        let json = r#"{"id":1,"is_obsolete":true,"is_private":false}"#;
        let att: Attachment = serde_json::from_str(json).unwrap();
        assert!(att.is_obsolete);
        assert!(!att.is_private);
    }

    #[test]
    fn bool_from_int_or_bool_deserializes_integers() {
        let json = r#"{"id":1,"is_obsolete":1,"is_private":0}"#;
        let att: Attachment = serde_json::from_str(json).unwrap();
        assert!(att.is_obsolete);
        assert!(!att.is_private);
    }

    #[test]
    fn bool_from_int_or_bool_rejects_string() {
        let json = r#"{"id":1,"is_obsolete":"yes"}"#;
        let err = serde_json::from_str::<Attachment>(json).unwrap_err();
        assert!(
            err.to_string().contains("expected bool or integer"),
            "unexpected error: {err}"
        );
    }
}
