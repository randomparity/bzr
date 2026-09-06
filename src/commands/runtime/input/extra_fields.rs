//! Parsing and merging of the `--field KEY=VALUE` / `--field-json` arbitrary
//! field passthrough accepted by `bug create` and `bug update` (ADR 0053).
//!
//! Parsing only decides the *shape* of the map. Whether the server declares
//! each key is settled later, against the field catalogue, by
//! [`crate::commands::runtime::shared::field_catalogue`].

use std::collections::{BTreeMap, BTreeSet};

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
        // Neither the message nor the structured `value` echoes the argument's
        // value half: a field value can be a secret, and ADR 0007 puts this
        // object on stderr where agents log it.
        return Err(BzrError::input_field(
            "--field: an argument has an empty field name before its '='".to_string(),
            "--field",
            None,
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
    // stdin can only be consumed once. When another flag on the same command
    // line already read it (`--from-json -`, `--description -`, `--comment -`,
    // or a piped description), this read comes back empty; say so, rather than
    // reporting an EOF parse error that names neither flag.
    if raw.trim().is_empty() {
        return Err(BzrError::input_field(
            format!(
                "--field-json: '{source}' produced an empty document. stdin can only be \
                 read once, so `--field-json -` cannot be combined with another flag \
                 that reads it (--from-json -, --description -, --description-file -, \
                 --comment -, --comment-file -, or a piped bug description); pass a \
                 file path instead"
            ),
            "--field-json",
            Some(source.to_string()),
        ));
    }
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
pub(crate) fn parse(pairs: &[String], json_source: Option<&str>) -> Result<ExtraFields> {
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
pub(crate) fn check_against<T: serde::Serialize>(
    typed: &T,
    extra: ExtraFields,
) -> Result<ExtraFields> {
    if extra.is_empty() {
        return Ok(extra);
    }
    let Ok(Value::Object(rendered)) = serde_json::to_value(typed) else {
        return Ok(extra);
    };
    for key in extra.keys() {
        if rendered.contains_key(key) {
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
    Ok(extra)
}

/// Reject `--field-json -` alongside another flag on the same command line that
/// also reads stdin.
///
/// stdin is consumable once, so one of the two would come back empty. Catching
/// the combination up front names both flags; the empty-document diagnostic in
/// [`parse`] is the fallback for the sources this cannot see, such as a bug
/// description arriving on a pipe with no flag of its own.
pub(crate) fn reject_stdin_conflict(
    json_source: Option<&str>,
    competing: &[(&str, bool)],
) -> Result<()> {
    if json_source != Some("-") {
        return Ok(());
    }
    for (flag, reads_stdin) in competing {
        if *reads_stdin {
            return Err(BzrError::input(format!(
                "--field-json - cannot be combined with {flag}: stdin can only be read \
                 once. Pass one of them a file path instead."
            )));
        }
    }
    Ok(())
}

/// The union of keys across one or more payloads' extra-field maps, for the
/// single pre-dispatch catalogue check that covers a whole batch.
pub(crate) fn key_union<'a>(maps: impl Iterator<Item = &'a ExtraFields>) -> BTreeSet<String> {
    maps.flat_map(|map| map.keys().cloned()).collect()
}

#[cfg(test)]
#[path = "extra_fields_tests.rs"]
mod tests;
