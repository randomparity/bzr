use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::xmlrpc::value::Value;

/// Error message used whenever an XML-RPC method expects a top-level struct
/// in the response but receives a different value type.
pub(crate) const EXPECTED_STRUCT_RESPONSE: &str = "expected struct response";

/// Returns a struct member as a bool, accepting either `<boolean>1</boolean>`
/// or `<int>1</int>` on the wire. Bugzilla 5.0.x XML-RPC responses use both
/// shapes interchangeably for the same flag depending on the field.
pub(crate) fn get_bool_flag(m: &BTreeMap<String, Value>, key: &str) -> bool {
    match m.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Int(n)) => *n != 0,
        _ => false,
    }
}

pub(crate) fn get_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(String::from)
}

pub(crate) fn get_nonempty_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    let val = m.get(key)?;
    match val {
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

pub(crate) fn get_datetime_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    let val = m.get(key)?;
    match val {
        Value::DateTime(s) => Some(s.clone()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

pub(crate) fn get_u64(m: &BTreeMap<String, Value>, key: &str) -> Option<u64> {
    m.get(key)
        .and_then(Value::as_i64)
        .and_then(|v| u64::try_from(v).ok())
}

pub(crate) fn get_str_array(m: &BTreeMap<String, Value>, key: &str) -> Vec<String> {
    m.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn get_int_array(m: &BTreeMap<String, Value>, key: &str) -> Vec<u64> {
    m.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_i64)
                .map(|v| {
                    #[expect(clippy::cast_sign_loss, reason = "bug IDs are non-negative")]
                    let id = v as u64;
                    id
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Navigate a `Bug.*` XML-RPC response from `top -> bugs -> {bug_id_str}`.
///
/// Returns `Ok(None)` when the server acknowledged the call but didn't
/// return data for this bug (caller should treat as empty result, not
/// error). Returns `Err` when the response shape is malformed.
pub(crate) fn lookup_bug_entry(response: &Value, bug_id: u64) -> Result<Option<&Value>> {
    let top = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;

    let Some(bugs_val) = top.get("bugs") else {
        return Ok(None);
    };

    let bugs_struct = bugs_val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected bugs to be a struct keyed by bug ID".into()))?;

    // Bugzilla returns the inner key as a string even though the input is
    // an integer. Look up by string form.
    Ok(bugs_struct.get(&bug_id.to_string()))
}

pub(crate) fn xmlrpc_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::String(s) | Value::DateTime(s) => serde_json::Value::String(s.clone()),
        Value::Int(n) => serde_json::Value::Number(serde_json::Number::from(*n)),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Double(n) => serde_json::Number::from_f64(*n).map_or_else(
            || serde_json::Value::String(n.to_string()),
            serde_json::Value::Number,
        ),
        Value::Base64(bytes) => serde_json::Value::String(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        )),
        Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(xmlrpc_value_to_json).collect())
        }
        Value::Struct(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(name, value)| (name.clone(), xmlrpc_value_to_json(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
#[path = "mappers_tests.rs"]
mod tests;
