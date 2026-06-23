use serde::de::DeserializeOwned;

use crate::error::{BzrError, Result};

#[derive(Debug)]
pub(crate) enum JsonOneOrMany<T> {
    One(Box<T>),
    Many(Vec<T>),
}

pub(crate) fn read_object<T: DeserializeOwned>(arg: &str) -> Result<T> {
    let raw = read_raw(arg)?;
    parse_object(&raw)
}

pub(crate) fn read_one_or_many<T: DeserializeOwned>(arg: &str) -> Result<JsonOneOrMany<T>> {
    let raw = read_raw(arg)?;
    parse_one_or_many(&raw)
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

fn read_raw(arg: &str) -> Result<String> {
    if arg == "-" {
        crate::commands::runtime::shared::read_stdin_to_string("read --from-json from stdin")
    } else {
        crate::commands::runtime::shared::read_file_with_context(
            std::path::Path::new(arg),
            "--from-json",
        )
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

pub(crate) fn parse_one_or_many<T: DeserializeOwned>(raw: &str) -> Result<JsonOneOrMany<T>> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| BzrError::InputValidation(format!("--from-json: invalid JSON: {e}")))?;
    match value {
        serde_json::Value::Object(_) => {
            let one = serde_json::from_value(value)
                .map_err(|e| BzrError::InputValidation(format!("--from-json: {e}")))?;
            Ok(JsonOneOrMany::One(Box::new(one)))
        }
        serde_json::Value::Array(items) => {
            let entries = items
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    serde_json::from_value(v).map_err(|e| {
                        BzrError::InputValidation(format!("--from-json item {i}: {e}"))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(JsonOneOrMany::Many(entries))
        }
        _ => Err(BzrError::InputValidation(
            "--from-json expects a JSON object or an array of objects".into(),
        )),
    }
}
