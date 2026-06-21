use serde::de::DeserializeOwned;

use crate::error::{BzrError, Result};

pub(crate) fn read_object<T: DeserializeOwned>(arg: &str) -> Result<T> {
    let raw = if arg == "-" {
        crate::commands::runtime::shared::read_stdin_to_string()?
    } else {
        crate::commands::runtime::shared::read_file_with_context(
            std::path::Path::new(arg),
            "--from-json",
        )?
    };
    parse_object(&raw)
}

pub(crate) fn merge_string(target: &mut Option<String>, value: Option<&str>) {
    if let Some(value) = value {
        *target = Some(value.to_string());
    }
}

pub(crate) fn merge_copy<T: Copy>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

pub(crate) fn required_string(value: Option<String>, field: &str) -> Result<String> {
    let flag = field.replace('_', "-");
    value.ok_or_else(|| {
        BzrError::InputValidation(format!(
            "--from-json: '{field}' is required (set it in the JSON or via --{flag})"
        ))
    })
}

pub(crate) fn resolve_string_target(
    positional: Option<&str>,
    json: Option<String>,
    conflict: &str,
    missing: &str,
) -> Result<String> {
    match (positional, json) {
        (Some(_), Some(_)) => Err(BzrError::InputValidation(conflict.into())),
        (Some(value), None) => Ok(value.to_string()),
        (None, Some(value)) => Ok(value),
        (None, None) => Err(BzrError::InputValidation(missing.into())),
    }
}

pub(crate) fn resolve_u64_target(
    positional: Option<u64>,
    json: Option<u64>,
    conflict: &str,
    missing: &str,
) -> Result<u64> {
    match (positional, json) {
        (Some(_), Some(_)) => Err(BzrError::InputValidation(conflict.into())),
        (Some(value), None) | (None, Some(value)) => Ok(value),
        (None, None) => Err(BzrError::InputValidation(missing.into())),
    }
}

fn parse_object<T: DeserializeOwned>(raw: &str) -> Result<T> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| BzrError::InputValidation(format!("--from-json: invalid JSON: {e}")))?;
    match value {
        serde_json::Value::Object(_) => serde_json::from_value(value)
            .map_err(|e| BzrError::InputValidation(format!("--from-json: {e}"))),
        _ => Err(BzrError::InputValidation(
            "--from-json expects a JSON object".into(),
        )),
    }
}
