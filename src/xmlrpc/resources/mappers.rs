use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::flag::Flag;
use crate::xmlrpc::protocol::Value;

/// Error message used whenever an XML-RPC method expects a top-level struct
/// in the response but receives a different value type.
pub(crate) const EXPECTED_STRUCT_RESPONSE: &str = "expected struct response";

pub(crate) fn get_optional_bool_flag(m: &BTreeMap<String, Value>, key: &str) -> Option<bool> {
    match m.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::Int(n)) => Some(*n != 0),
        _ => None,
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

/// Extract a required non-negative `u64` ID from a struct member.
///
/// XML-RPC transmits integers as signed `i64`, but domain identifiers
/// (bug/comment/attachment/user/group) are non-negative. A missing field or a
/// negative value is a malformed response, so this errors rather than silently
/// wrapping the bit pattern the way an `as u64` cast would. `resource` names the
/// object for the error message (e.g. `"bug"`).
pub(crate) fn require_u64(m: &BTreeMap<String, Value>, key: &str, resource: &str) -> Result<u64> {
    let v = m
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc(format!("{resource} missing {key} field")))?;
    u64::try_from(v).map_err(|_| BzrError::XmlRpc(format!("{resource} {key} is negative: {v}")))
}

pub(crate) fn xmlrpc_id(value: u64, label: &str) -> Result<Value> {
    i64::try_from(value).map(Value::Int).map_err(|_| {
        BzrError::input(format!(
            "{label} {value} is outside the XML-RPC signed integer range 0..={}",
            i64::MAX
        ))
    })
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

/// Parse a `flags` array of flag structs into view-side [`Flag`] objects.
/// Non-struct array elements are skipped; missing members stay `None`, matching
/// the REST deserializer's tolerance without inventing empty names/statuses.
pub(crate) fn get_flags(m: &BTreeMap<String, Value>, key: &str) -> Vec<Flag> {
    m.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_struct)
                .map(|fm| Flag {
                    name: get_nonempty_str(fm, "name"),
                    status: get_nonempty_str(fm, "status"),
                    setter: get_nonempty_str(fm, "setter"),
                    requestee: get_nonempty_str(fm, "requestee"),
                })
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
                .filter_map(|v| u64::try_from(v).ok())
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
