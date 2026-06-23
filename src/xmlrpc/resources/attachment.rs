use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;
use crate::xmlrpc::resources::mappers::{
    get_bool_flag, get_datetime_str, get_flags, get_nonempty_str, get_str, get_u64,
    lookup_bug_entry, require_u64, EXPECTED_STRUCT_RESPONSE,
};

const ATTACHMENT_LIST_FIELDS: &[&str] = &[
    "id",
    "bug_id",
    "file_name",
    "summary",
    "content_type",
    "creator",
    "creation_time",
    "last_change_time",
    "size",
    "is_obsolete",
    "is_private",
    "is_patch",
    "data",
];

impl XmlRpcClient {
    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<crate::types::Attachment>> {
        let mut rpc_params = BTreeMap::new();
        #[expect(clippy::cast_possible_wrap, reason = "bug IDs fit in i64")]
        let bug_id_value = Value::Int(bug_id as i64);
        rpc_params.insert("ids".into(), Value::Array(vec![bug_id_value]));
        let include_fields = ATTACHMENT_LIST_FIELDS
            .iter()
            .copied()
            .map(Value::from)
            .collect();
        rpc_params.insert("include_fields".into(), Value::Array(include_fields));

        let result = self.call("Bug.attachments", rpc_params).await?;
        extract_attachments(&result, bug_id)
    }

    pub async fn get_attachment_by_id(
        &self,
        attachment_id: u64,
    ) -> Result<crate::types::Attachment> {
        let mut rpc_params = BTreeMap::new();
        #[expect(clippy::cast_possible_wrap, reason = "attachment IDs fit in i64")]
        let id_value = Value::Int(attachment_id as i64);
        rpc_params.insert("attachment_ids".into(), Value::Array(vec![id_value]));

        let result = self.call("Bug.attachments", rpc_params).await?;
        extract_attachment_by_id(&result, attachment_id)
    }
}

fn extract_attachments(response: &Value, bug_id: u64) -> Result<Vec<crate::types::Attachment>> {
    let Some(bug_entry) = lookup_bug_entry(response, bug_id)? else {
        return Ok(Vec::new());
    };

    let attachments_arr = bug_entry
        .as_array()
        .ok_or_else(|| BzrError::XmlRpc("expected attachments array".into()))?;

    let mut attachments = Vec::with_capacity(attachments_arr.len());
    for a in attachments_arr {
        attachments.push(value_to_attachment(a)?);
    }
    Ok(attachments)
}

fn extract_attachment_by_id(
    response: &Value,
    attachment_id: u64,
) -> Result<crate::types::Attachment> {
    let top = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;

    let attachments_struct = top
        .get("attachments")
        .and_then(Value::as_struct)
        .ok_or_else(|| {
            BzrError::XmlRpc("expected attachments to be a struct keyed by attachment ID".into())
        })?;

    let key = attachment_id.to_string();
    let entry = attachments_struct
        .get(&key)
        .ok_or_else(|| BzrError::NotFound {
            resource: "attachment",
            id: attachment_id.to_string(),
        })?;

    value_to_attachment(entry)
}

fn value_to_attachment(val: &Value) -> Result<crate::types::Attachment> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for attachment".into()))?;

    let data = match m.get("data") {
        Some(Value::Base64(bytes)) => Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        )),
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };

    Ok(crate::types::Attachment {
        // `id` is a primary key and must be present and non-negative; secondary
        // numeric fields (`bug_id`, `size`) default to 0 because off-spec
        // Bugzilla envelopes may omit them, and a zero is harmless for display.
        id: require_u64(m, "id", "attachment")?,
        bug_id: get_u64(m, "bug_id").unwrap_or(0),
        file_name: get_str(m, "file_name").unwrap_or_default(),
        summary: get_str(m, "summary").unwrap_or_default(),
        content_type: get_str(m, "content_type").unwrap_or_default(),
        creator: get_nonempty_str(m, "creator"),
        creation_time: get_datetime_str(m, "creation_time"),
        last_change_time: get_datetime_str(m, "last_change_time"),
        size: get_u64(m, "size").unwrap_or(0),
        is_obsolete: get_bool_flag(m, "is_obsolete"),
        is_private: get_bool_flag(m, "is_private"),
        is_patch: get_bool_flag(m, "is_patch"),
        flags: get_flags(m, "flags"),
        data,
    })
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
