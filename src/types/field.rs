use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Known field name aliases mapped to their Bugzilla API internal names.
/// Sorted alphabetically by alias.
///
/// These aliases cannot shadow real Bugzilla field names because Bugzilla
/// requires custom fields to use the `cf_` prefix (e.g. `cf_status`), and
/// the built-in fields have fixed names (e.g. `bug_status`, `priority`).
/// No real field can have a bare name like `status` or `severity`, so eager
/// resolution is always safe.
pub(crate) const FIELD_ALIASES: &[(&str, &str)] = &[
    ("file_loc", "bug_file_loc"),
    ("group", "bug_group"),
    ("id", "bug_id"),
    ("severity", "bug_severity"),
    ("status", "bug_status"),
    ("type", "bug_type"),
];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FieldValue {
    /// Field value name. Null for the "default/unset" entry in some Bugzilla
    /// field types (e.g. `bug_status` on Bugzilla 5.0 has a null-named entry).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::types::sort_key::deserialize_optional",
        serialize_with = "crate::types::sort_key::serialize_optional"
    )]
    pub sort_key: Option<i128>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub can_change_to: Option<Vec<StatusTransition>>,
}

/// Serde JSON keys of [`FieldValue`], for `--fields` / `--exclude-fields`
/// validation on `field list`.
pub const FIELD_VALUE_FIELDS: &[&str] = &["name", "sort_key", "is_active", "can_change_to"];

/// Why a bug field name is accepted by `--field` / `--field-json` (ADR 0062).
///
/// [`FieldNameSource::as_str`] is the single definition of the three spellings:
/// serde serializes through it via `#[serde(into = "&'static str")]`, and the
/// table writer calls it directly, so the JSON and table output cannot name a
/// source differently. `schemas/field-name.json` pins the same three values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum FieldNameSource {
    /// The connected server's `field/bug` catalogue declares it.
    Server,
    /// bzr models it as a canonical REST bug field (`BUG_FIELDS`).
    Bzr,
    /// Both sources name it.
    Both,
}

impl FieldNameSource {
    /// The wire and table spelling. The one definition; see the type docs.
    pub fn as_str(self) -> &'static str {
        match self {
            FieldNameSource::Server => "server",
            FieldNameSource::Bzr => "bzr",
            FieldNameSource::Both => "both",
        }
    }
}

impl From<FieldNameSource> for &'static str {
    fn from(source: FieldNameSource) -> Self {
        source.as_str()
    }
}

/// A bug field name `bzr bug create` / `bzr bug update` accept for `--field` /
/// `--field-json`, as emitted by `bzr field list` with no positional argument.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct FieldName {
    pub name: String,
    pub source: FieldNameSource,
}

/// Serde JSON keys of [`FieldName`], for `--fields` / `--exclude-fields`
/// validation on the no-argument `field list`.
pub const FIELD_NAME_FIELDS: &[&str] = &["name", "source"];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusTransition {
    pub name: String,
}

pub(crate) fn resolve_field_alias(name: &str) -> Cow<'_, str> {
    let lower = name.to_ascii_lowercase();
    for &(alias, api_name) in FIELD_ALIASES {
        if lower == alias {
            return Cow::Borrowed(api_name);
        }
    }
    // Unknown fields pass through unchanged; only known aliases are normalized.
    Cow::Borrowed(name)
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
