use serde::{Deserialize, Deserializer, Serialize};

use super::common::FlagUpdate;

/// Generates a match expression mapping `FIELD_MAPPINGS` `struct_field` names
/// to struct fields. Used by `SearchParams::get_field` and
/// `SavedQuery::get_field_mut` to keep both in sync with a single definition.
macro_rules! match_field {
    ($name:expr, $self:expr, $wrap:ident, $default:expr,
     { $($field:literal => $member:ident),+ $(,)? }) => {
        match $name {
            $($field => $wrap!($self.$member),)+
            _ => $default,
        }
    };
}

/// Deserialize a string that may be null into an empty string.
fn deserialize_null_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(d).map(Option::unwrap_or_default)
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bug {
    pub id: u64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub assigned_to: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub whiteboard: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<u64>,
    #[serde(default)]
    pub depends_on: Vec<u64>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub op_sys: Option<String>,
    #[serde(default)]
    pub rep_platform: Option<String>,
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SearchParams {
    pub product: Vec<String>,
    pub component: Vec<String>,
    pub status: Vec<String>,
    pub assigned_to: Vec<String>,
    pub creator: Vec<String>,
    pub priority: Vec<String>,
    pub severity: Vec<String>,
    pub cc: Option<String>,
    pub alias: Option<String>,
    /// Bug IDs to search for.
    pub id: Vec<u64>,
    pub limit: Option<u32>,
    pub summary: Option<String>,
    pub quicksearch: Option<String>,
    pub include_fields: Option<String>,
    pub exclude_fields: Option<String>,
    /// Raw query parameters passed through verbatim to the REST API.
    /// Used for URL-imported queries with boolean chart params.
    pub raw_params: Vec<(String, String)>,
    /// Filter to bugs created at or after this datetime (server-canonical
    /// form: e.g. `2026-04-01T00:00:00Z`). Validated client-side at the
    /// CLI layer via `crate::validation::parse_iso8601_or_date`.
    pub creation_time: Option<String>,
    /// Filter to bugs last modified at or after this datetime (server-canonical
    /// form). Validated client-side; see `creation_time`.
    pub last_change_time: Option<String>,
}

impl SearchParams {
    /// Apply optional runtime overrides for limit, fields, `exclude_fields`,
    /// and the two date filters. `Some(_)` replaces; `None` keeps the
    /// saved value.
    pub fn apply_overrides(
        &mut self,
        limit: Option<u32>,
        fields: Option<&str>,
        exclude_fields: Option<&str>,
        creation_time: Option<&str>,
        last_change_time: Option<&str>,
    ) {
        if let Some(l) = limit {
            self.limit = Some(l);
        }
        if let Some(f) = fields {
            self.include_fields = Some(f.to_string());
        }
        if let Some(ef) = exclude_fields {
            self.exclude_fields = Some(ef.to_string());
        }
        if let Some(ct) = creation_time {
            self.creation_time = Some(ct.to_string());
        }
        if let Some(lct) = last_change_time {
            self.last_change_time = Some(lct.to_string());
        }
    }

    /// Access a multi-value filter field by its `struct_field` name.
    ///
    /// # Panics
    ///
    /// Panics if `name` is not one of the 7 known field names in
    /// `FIELD_MAPPINGS`. Only called with compile-time-known names.
    pub fn get_field(&self, name: &str) -> &[String] {
        macro_rules! as_ref {
            ($e:expr) => {
                &$e
            };
        }
        match_field!(name, self, as_ref, panic!("unknown field: {name}"), {
            "product" => product,
            "component" => component,
            "status" => status,
            "assigned_to" => assigned_to,
            "creator" => creator,
            "priority" => priority,
            "severity" => severity,
        })
    }

    /// Returns true if any filter fields are set (product, component, etc.).
    ///
    /// Note: `limit`, `include_fields`, and `exclude_fields` are intentionally
    /// excluded — they control pagination and field selection, not bug filtering.
    pub fn has_filters(&self) -> bool {
        !self.product.is_empty()
            || !self.component.is_empty()
            || !self.status.is_empty()
            || !self.assigned_to.is_empty()
            || !self.creator.is_empty()
            || !self.priority.is_empty()
            || !self.severity.is_empty()
            || self.cc.is_some()
            || self.alias.is_some()
            || !self.id.is_empty()
            || self.summary.is_some()
            || self.quicksearch.is_some()
            || !self.raw_params.is_empty()
            || self.creation_time.is_some()
            || self.last_change_time.is_some()
    }

    /// Returns true if any *structured* filter is set.
    ///
    /// Differs from [`Self::has_filters`] by excluding `quicksearch` and
    /// `summary`, which are free-text predicates evaluated by the same
    /// server-side parser regardless of transport (REST vs XML-RPC).
    ///
    /// Used by hybrid mode to decide whether an empty REST result warrants
    /// an XML-RPC retry: only structured filters are retried, since they
    /// are the cases where a buggy REST extension can disagree with the
    /// XML-RPC implementation. An empty quicksearch or summary result is
    /// authoritative — retrying via XML-RPC will return the same set
    /// (and may incur a long timeout on servers with slow XML-RPC).
    pub fn has_structured_filters(&self) -> bool {
        !self.product.is_empty()
            || !self.component.is_empty()
            || !self.status.is_empty()
            || !self.assigned_to.is_empty()
            || !self.creator.is_empty()
            || !self.priority.is_empty()
            || !self.severity.is_empty()
            || self.cc.is_some()
            || self.alias.is_some()
            || !self.id.is_empty()
            || !self.raw_params.is_empty()
            || self.creation_time.is_some()
            || self.last_change_time.is_some()
    }
}

/// Splits filter values into (positive, negated) groups.
/// Values prefixed with `!` are negated; the prefix is stripped.
pub fn partition_filters(values: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut positive = Vec::new();
    let mut negated = Vec::new();
    for v in values {
        if let Some(stripped) = v.strip_prefix('!') {
            negated.push(stripped);
        } else {
            positive.push(v.as_str());
        }
    }
    (positive, negated)
}

/// Maps a filterable field across all naming contexts.
pub struct FieldMapping {
    /// Name on `SearchParams` / `SavedQuery` (e.g. "status").
    /// Also used as the REST API query parameter.
    pub struct_field: &'static str,
    /// `buglist.cgi` URL parameter name (e.g. `bug_status`).
    pub url_param: &'static str,
    /// Bugzilla internal name for boolean charts (e.g. `bug_status`).
    pub internal_name: &'static str,
}

/// Canonical field-mapping table for the 7 multi-value filter fields.
pub const FIELD_MAPPINGS: &[FieldMapping] = &[
    FieldMapping {
        struct_field: "product",
        url_param: "product",
        internal_name: "product",
    },
    FieldMapping {
        struct_field: "component",
        url_param: "component",
        internal_name: "component",
    },
    FieldMapping {
        struct_field: "status",
        url_param: "bug_status",
        internal_name: "bug_status",
    },
    FieldMapping {
        struct_field: "assigned_to",
        url_param: "assigned_to",
        internal_name: "assigned_to",
    },
    FieldMapping {
        struct_field: "creator",
        url_param: "reporter",
        internal_name: "reporter",
    },
    FieldMapping {
        struct_field: "priority",
        url_param: "priority",
        internal_name: "priority",
    },
    FieldMapping {
        struct_field: "severity",
        url_param: "bug_severity",
        internal_name: "bug_severity",
    },
];

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateBugParams {
    pub product: String,
    pub component: String,
    pub summary: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_sys: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rep_platform: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

/// Represents an incremental update to a list field (blocks, `depends_on`).
/// Bugzilla accepts `{ "add": [...], "remove": [...] }` for these fields.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct IdListUpdate {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<u64>,
}

impl IdListUpdate {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateBugParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
    #[serde(skip_serializing_if = "IdListUpdate::is_empty")]
    pub blocks: IdListUpdate,
    #[serde(skip_serializing_if = "IdListUpdate::is_empty")]
    pub depends_on: IdListUpdate,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HistoryEntry {
    pub who: String,
    pub when: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FieldChange {
    pub field_name: String,
    #[serde(default)]
    pub removed: String,
    #[serde(default)]
    pub added: String,
    #[serde(default)]
    pub attachment_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FieldValue {
    /// Field value name. Null for the "default/unset" entry in some Bugzilla
    /// field types (e.g. `bug_status` on Bugzilla 5.0 has a null-named entry).
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub can_change_to: Option<Vec<StatusTransition>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusTransition {
    pub name: String,
}

/// The kind of saved query — determines which fields are meaningful.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QueryKind {
    /// Structured filter query (product, status, etc.)
    #[default]
    List,
    /// Free-text quicksearch query
    Search,
    /// Query imported from a Bugzilla URL (may contain raw passthrough params)
    Url,
}

/// A reusable bug query stored in the config file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedQuery {
    #[serde(default)]
    pub kind: QueryKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub product: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignee: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub creator: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub severity: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quicksearch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_fields: Option<String>,
    /// The original Bugzilla URL this query was parsed from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Server name (from config) this query is associated with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Raw query parameters not mapped to structured fields.
    /// Passed through verbatim to the Bugzilla REST API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_params: Vec<(String, String)>,
    /// Server-canonical form (e.g. `2026-04-01T00:00:00Z`).
    /// Validated at save time via `crate::validation::parse_iso8601_or_date`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    /// Server-canonical form. See `creation_time`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_change_time: Option<String>,
}

impl SavedQuery {
    /// Convert this saved query into `SearchParams` by cloning.
    pub fn to_search_params(&self) -> SearchParams {
        self.clone().into_search_params()
    }

    /// Convert this saved query into `SearchParams`, consuming `self`.
    pub fn into_search_params(self) -> SearchParams {
        SearchParams {
            product: self.product,
            component: self.component,
            status: self.status,
            assigned_to: self.assignee,
            creator: self.creator,
            priority: self.priority,
            severity: self.severity,
            quicksearch: self.quicksearch,
            limit: self.limit,
            include_fields: self.fields,
            exclude_fields: self.exclude_fields,
            raw_params: self.raw_params,
            creation_time: self.creation_time,
            last_change_time: self.last_change_time,
            ..Default::default()
        }
    }

    /// Access a multi-value filter field mutably by its `struct_field` name.
    /// Maps `assigned_to` to `self.assignee` (TOML-friendly name).
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Vec<String>> {
        macro_rules! some_mut {
            ($e:expr) => {
                Some(&mut $e)
            };
        }
        match_field!(name, self, some_mut, None, {
            "product" => product,
            "component" => component,
            "status" => status,
            "assigned_to" => assignee,
            "creator" => creator,
            "priority" => priority,
            "severity" => severity,
        })
    }

    /// Returns true if the query has any meaningful filters set.
    pub fn has_filters(&self) -> bool {
        !self.product.is_empty()
            || !self.component.is_empty()
            || !self.status.is_empty()
            || !self.assignee.is_empty()
            || !self.creator.is_empty()
            || !self.priority.is_empty()
            || !self.severity.is_empty()
            || self.quicksearch.is_some()
            || !self.raw_params.is_empty()
            || self.creation_time.is_some()
            || self.last_change_time.is_some()
    }
}

/// A named set of default field values for bug creation.
/// Defined in `types::bug` because it represents a domain concept
/// (bug creation defaults), not configuration infrastructure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BugTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_sys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rep_platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
