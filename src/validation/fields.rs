use std::collections::BTreeSet;
use std::io::Write;

use crate::error::{BzrError, Result};
use crate::types::output::OutputFormat;

/// A validated field selection ready to apply to serialized JSON. The identity
/// projection ([`FieldProjection::none`]) passes every key through.
#[derive(Debug, Clone, Default)]
pub struct FieldProjection {
    /// Keys to keep. `None` means "keep all" (no `--fields` given).
    include: Option<BTreeSet<String>>,
    /// Keys to drop, applied after `include`.
    exclude: BTreeSet<String>,
}

impl FieldProjection {
    /// Identity projection.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Parse and validate `--fields` / `--exclude-fields` against `known` serde
    /// keys. Tokens are trimmed; blanks skipped; duplicates collapsed.
    ///
    /// # Errors
    /// Returns [`BzrError::InputValidation`] (exit 7) when any include or
    /// exclude token is not in `known`, or when the resolved key set is empty.
    pub fn resolve(include: Option<&str>, exclude: Option<&str>, known: &[&str]) -> Result<Self> {
        let include_set = parse_tokens(include, known, "--fields")?;
        let exclude_set = parse_tokens(exclude, known, "--exclude-fields")?.unwrap_or_default();

        let effective: BTreeSet<String> = match &include_set {
            Some(inc) => inc.difference(&exclude_set).cloned().collect(),
            None => known
                .iter()
                .map(|k| (*k).to_string())
                .filter(|k| !exclude_set.contains(k))
                .collect(),
        };
        if effective.is_empty() {
            return Err(BzrError::input(
                "the field selection leaves no fields to emit; \
             adjust --fields / --exclude-fields"
                    .into(),
            ));
        }
        Ok(Self {
            include: include_set,
            exclude: exclude_set,
        })
    }

    /// Would `key` be dropped by this projection?
    #[must_use]
    pub fn would_drop(&self, key: &str) -> bool {
        self.exclude.contains(key) || self.include.as_ref().is_some_and(|inc| !inc.contains(key))
    }

    /// Force `key` through regardless of the flags: into the include set when
    /// one exists, and out of the exclude set. For a field carrying meaning the
    /// caller cannot opt out of — `comment list`'s multi-ID `bug_id`, which is
    /// the only thing attributing a record to its bug, is the sole use today.
    pub fn retain_key(&mut self, key: &str) {
        if let Some(inc) = &mut self.include {
            inc.insert(key.to_string());
        }
        self.exclude.remove(key);
    }

    /// Project a serialized value in place: an object keeps/drops top-level
    /// keys; an array projects each element; other values are untouched.
    pub fn apply(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    self.apply(item);
                }
            }
            serde_json::Value::Object(map) => {
                if let Some(inc) = &self.include {
                    map.retain(|k, _| inc.contains(k));
                }
                for key in &self.exclude {
                    map.remove(key);
                }
            }
            _ => {}
        }
    }
}

/// Tokenize a comma list, validating every non-blank token against `known`.
/// Returns `None` when the input is absent or all-blank. `flag` names the
/// originating CLI flag (`--fields` / `--exclude-fields`) so a rejected token is
/// surfaced as the `field` / `value` attribution in `--json` error output.
fn parse_tokens(
    list: Option<&str>,
    known: &[&str],
    flag: &str,
) -> Result<Option<BTreeSet<String>>> {
    let Some(list) = list else {
        return Ok(None);
    };
    let mut out = BTreeSet::new();
    for token in list.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if !known.contains(&token) {
            return Err(BzrError::input_field(
                format!(
                    "unknown field '{token}'; known fields: {}",
                    known.join(", ")
                ),
                flag,
                Some(token.to_string()),
            ));
        }
        out.insert(token.to_string());
    }
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

/// Resolve a projection for a command: in the JSON family, validate (exit 7 on
/// unknown/empty); in table mode, warn once on `err` if either flag was given
/// and return the identity projection.
///
/// # Errors
/// Propagates [`FieldProjection::resolve`] validation errors in the JSON family.
pub fn projection_for<W: Write + ?Sized>(
    format: OutputFormat,
    include: Option<&str>,
    exclude: Option<&str>,
    known: &[&str],
    err: &mut W,
) -> Result<FieldProjection> {
    if format.is_json_family() {
        FieldProjection::resolve(include, exclude, known)
    } else {
        if include.is_some() || exclude.is_some() {
            let _ = writeln!(
                err,
                "warning: --fields/--exclude-fields only affect --json/--output \
                 ndjson; ignoring for table output"
            );
        }
        Ok(FieldProjection::none())
    }
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
