//! Parsing and merging of the `--field KEY=VALUE` / `--field-json` arbitrary
//! field passthrough accepted by `bug create` and `bug update` (ADR 0053).
//!
//! Parsing only decides the *shape* of the map. Whether the server declares
//! each key is settled later, against the field catalogue, by
//! [`crate::commands::runtime::shared::field_catalogue`].

use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::{BzrError, Result};

/// The merged `--field` / `--field-json` map. `BTreeMap` so the wire payload
/// and every diagnostic order keys deterministically.
pub(crate) type ExtraFields = BTreeMap<String, Value>;

fn duplicate(key: &str) -> BzrError {
    BzrError::input_field(
        format!("--field: '{key}' was supplied more than once; set each field once"),
        "--field",
        Some(key.to_string()),
    )
}

/// Insert `key`, rejecting a key that a previous `--field` or `--field-json`
/// entry already set.
fn insert_unique(out: &mut ExtraFields, key: String, value: Value) -> Result<()> {
    if out.contains_key(&key) {
        return Err(duplicate(&key));
    }
    out.insert(key, value);
    Ok(())
}

/// Split one `--field` argument on its first `=`. Everything after it is the
/// value, so `--field key=a=b` sets `a=b` and `--field key=` sets the empty
/// string, which is how Bugzilla clears a field.
fn parse_pair(raw: &str) -> Result<(String, Value)> {
    let (key, value) = raw.split_once('=').ok_or_else(|| {
        BzrError::input_field(
            format!("--field: '{raw}' is not KEY=VALUE (expected an '=' separator)"),
            "--field",
            Some(raw.to_string()),
        )
    })?;
    let key = key.trim();
    if key.is_empty() {
        return Err(BzrError::input_field(
            format!("--field: '{raw}' has an empty field name"),
            "--field",
            Some(raw.to_string()),
        ));
    }
    Ok((key.to_string(), Value::String(value.to_string())))
}

/// Read `--field-json` (a path, or `-` for stdin) as a JSON object.
fn parse_json_source(source: &str) -> Result<ExtraFields> {
    let raw = if source == "-" {
        crate::commands::runtime::shared::read_stdin_to_string("read --field-json from stdin")?
    } else {
        crate::commands::runtime::shared::read_file_with_context(
            std::path::Path::new(source),
            "--field-json",
        )?
    };
    let value: Value = serde_json::from_str(&raw).map_err(|e| {
        BzrError::input_field(
            format!("--field-json: '{source}' is not valid JSON: {e}"),
            "--field-json",
            Some(source.to_string()),
        )
    })?;
    let Value::Object(map) = value else {
        return Err(BzrError::input_field(
            format!("--field-json: '{source}' must contain a JSON object of field names to values"),
            "--field-json",
            Some(source.to_string()),
        ));
    };
    let mut out = ExtraFields::new();
    for (key, value) in map {
        if key.trim().is_empty() {
            return Err(BzrError::input_field(
                format!("--field-json: '{source}' has an empty field name"),
                "--field-json",
                Some(source.to_string()),
            ));
        }
        insert_unique(&mut out, key, value)?;
    }
    Ok(out)
}

/// Merge the repeatable `--field KEY=VALUE` arguments with an optional
/// `--field-json` source into one map. A key set by both, or by `--field`
/// twice, is rejected rather than silently resolved (exit 7).
pub(crate) fn parse_extra_fields(
    pairs: &[String],
    json_source: Option<&str>,
) -> Result<ExtraFields> {
    let mut out = match json_source {
        Some(source) => parse_json_source(source)?,
        None => ExtraFields::new(),
    };
    for raw in pairs {
        let (key, value) = parse_pair(raw)?;
        insert_unique(&mut out, key, value)?;
    }
    Ok(out)
}

/// Reject an extra field that the typed payload already sends.
///
/// `typed` is the built params *before* extras are attached; comparing against
/// its serialized form rather than a hand-maintained key list means the check
/// tracks `skip_serializing_if` and cannot drift from the payload structs.
/// `--field whiteboard=x` is therefore accepted when `--whiteboard` was not
/// given and rejected when it was.
pub(crate) fn reject_typed_collisions<T: serde::Serialize>(
    typed: &T,
    extra: &ExtraFields,
) -> Result<()> {
    if extra.is_empty() {
        return Ok(());
    }
    let Ok(Value::Object(typed)) = serde_json::to_value(typed) else {
        return Ok(());
    };
    for key in extra.keys() {
        if typed.contains_key(key) {
            return Err(BzrError::input_field(
                format!(
                    "--field: '{key}' is already set by this request's typed \
                     '{key}' field; use the dedicated flag instead"
                ),
                "--field",
                Some(key.clone()),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "extra_fields_tests.rs"]
mod tests;
