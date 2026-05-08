use serde::{Deserialize, Deserializer, Serialize};

use super::common::FlagUpdate;

#[derive(Clone, Copy)]
enum FilterField {
    Product,
    Component,
    Status,
    AssignedTo,
    Creator,
    Priority,
    Severity,
    Whiteboard,
    TargetMilestone,
    Version,
    OpSys,
    Platform,
    Resolution,
    QaContact,
    Url,
}

impl FilterField {
    fn from_struct_field(name: &str) -> Option<Self> {
        match name {
            "product" => Some(Self::Product),
            "component" => Some(Self::Component),
            "status" => Some(Self::Status),
            "assigned_to" => Some(Self::AssignedTo),
            "creator" => Some(Self::Creator),
            "priority" => Some(Self::Priority),
            "severity" => Some(Self::Severity),
            "whiteboard" => Some(Self::Whiteboard),
            "target_milestone" => Some(Self::TargetMilestone),
            "version" => Some(Self::Version),
            "op_sys" => Some(Self::OpSys),
            "platform" => Some(Self::Platform),
            "resolution" => Some(Self::Resolution),
            "qa_contact" => Some(Self::QaContact),
            "url" => Some(Self::Url),
            _ => None,
        }
    }
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
    pub dupe_of: Option<u64>,
    #[serde(default)]
    pub deadline: Option<String>,
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
    /// Filter by Status Whiteboard substring (repeatable). Negated
    /// values use `notsubstring`. Server-side substring matching is
    /// native to Bugzilla for this field.
    pub whiteboard: Vec<String>,
    /// Filter by Target Milestone (repeatable). Exact match.
    pub target_milestone: Vec<String>,
    /// Filter by Version (repeatable). Exact match.
    pub version: Vec<String>,
    /// Filter by Operating System (repeatable). Exact match.
    pub op_sys: Vec<String>,
    /// Filter by Platform / Hardware (repeatable). Exact match. The
    /// Bugzilla `Bug.search` API parameter is `platform` (the bug
    /// record field is `rep_platform`); we match the search-API
    /// name here.
    pub platform: Vec<String>,
    /// Filter by Resolution (repeatable). Exact match. Empty
    /// resolution matches open bugs.
    pub resolution: Vec<String>,
    /// Filter by QA Contact login (repeatable). Exact match.
    pub qa_contact: Vec<String>,
    /// Filter by URL field substring (repeatable). Negated values
    /// use `notsubstring`.
    pub url: Vec<String>,
}

/// Optional per-invocation overrides applied to a `SearchParams`
/// (typically constructed from a `SavedQuery` by `bzr query run`).
///
/// Each `None` keeps whatever the saved value was; each `Some(_)`
/// replaces it. Construct with `Overrides { limit,
/// ..Default::default() }` and only populate the fields you want to
/// override.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct Overrides<'a> {
    pub limit: Option<u32>,
    pub fields: Option<&'a str>,
    pub exclude_fields: Option<&'a str>,
    pub creation_time: Option<&'a str>,
    pub last_change_time: Option<&'a str>,
    pub whiteboard: Option<&'a [String]>,
    pub target_milestone: Option<&'a [String]>,
    pub version: Option<&'a [String]>,
    pub op_sys: Option<&'a [String]>,
    pub platform: Option<&'a [String]>,
    pub resolution: Option<&'a [String]>,
    pub qa_contact: Option<&'a [String]>,
    pub url: Option<&'a [String]>,
}

impl SearchParams {
    /// Apply optional per-invocation overrides. `Some(_)` replaces;
    /// `None` keeps the saved value.
    pub fn apply_overrides(&mut self, o: Overrides<'_>) {
        if let Some(l) = o.limit {
            self.limit = Some(l);
        }
        if let Some(f) = o.fields {
            self.include_fields = Some(f.to_string());
        }
        if let Some(ef) = o.exclude_fields {
            self.exclude_fields = Some(ef.to_string());
        }
        if let Some(ct) = o.creation_time {
            self.creation_time = Some(ct.to_string());
        }
        if let Some(lct) = o.last_change_time {
            self.last_change_time = Some(lct.to_string());
        }
        if let Some(v) = o.whiteboard {
            self.whiteboard = v.to_vec();
        }
        if let Some(v) = o.target_milestone {
            self.target_milestone = v.to_vec();
        }
        if let Some(v) = o.version {
            self.version = v.to_vec();
        }
        if let Some(v) = o.op_sys {
            self.op_sys = v.to_vec();
        }
        if let Some(v) = o.platform {
            self.platform = v.to_vec();
        }
        if let Some(v) = o.resolution {
            self.resolution = v.to_vec();
        }
        if let Some(v) = o.qa_contact {
            self.qa_contact = v.to_vec();
        }
        if let Some(v) = o.url {
            self.url = v.to_vec();
        }
    }

    /// Access a multi-value filter field by its `struct_field` name.
    pub(crate) fn get_field(&self, name: &str) -> Option<&[String]> {
        FilterField::from_struct_field(name).map(|field| self.get_filter_field(field))
    }

    /// Access a multi-value filter field mutably by its `struct_field` name.
    #[cfg(test)]
    pub(crate) fn get_field_mut(&mut self, name: &str) -> Option<&mut Vec<String>> {
        FilterField::from_struct_field(name).map(|field| self.get_filter_field_mut(field))
    }

    fn get_filter_field(&self, field: FilterField) -> &[String] {
        match field {
            FilterField::Product => &self.product,
            FilterField::Component => &self.component,
            FilterField::Status => &self.status,
            FilterField::AssignedTo => &self.assigned_to,
            FilterField::Creator => &self.creator,
            FilterField::Priority => &self.priority,
            FilterField::Severity => &self.severity,
            FilterField::Whiteboard => &self.whiteboard,
            FilterField::TargetMilestone => &self.target_milestone,
            FilterField::Version => &self.version,
            FilterField::OpSys => &self.op_sys,
            FilterField::Platform => &self.platform,
            FilterField::Resolution => &self.resolution,
            FilterField::QaContact => &self.qa_contact,
            FilterField::Url => &self.url,
        }
    }

    #[cfg(test)]
    fn get_filter_field_mut(&mut self, field: FilterField) -> &mut Vec<String> {
        match field {
            FilterField::Product => &mut self.product,
            FilterField::Component => &mut self.component,
            FilterField::Status => &mut self.status,
            FilterField::AssignedTo => &mut self.assigned_to,
            FilterField::Creator => &mut self.creator,
            FilterField::Priority => &mut self.priority,
            FilterField::Severity => &mut self.severity,
            FilterField::Whiteboard => &mut self.whiteboard,
            FilterField::TargetMilestone => &mut self.target_milestone,
            FilterField::Version => &mut self.version,
            FilterField::OpSys => &mut self.op_sys,
            FilterField::Platform => &mut self.platform,
            FilterField::Resolution => &mut self.resolution,
            FilterField::QaContact => &mut self.qa_contact,
            FilterField::Url => &mut self.url,
        }
    }

    fn has_mapped_filters(&self) -> bool {
        FIELD_MAPPINGS.iter().any(|mapping| {
            self.get_field(mapping.struct_field)
                .is_some_and(|field| !field.is_empty())
        })
    }

    /// Returns true if any filter fields are set (product, component, etc.).
    ///
    /// Note: `limit`, `include_fields`, and `exclude_fields` are intentionally
    /// excluded — they control pagination and field selection, not bug filtering.
    pub fn has_filters(&self) -> bool {
        self.has_mapped_filters()
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
        self.has_mapped_filters()
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

/// Bugzilla boolean-chart operator used when a filter value is
/// negated (`!`-prefix). Each `FieldMapping` row picks one based on
/// the field's positive-side match style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NegationOp {
    /// For exact-match fields (the inverse of `equals`).
    NotEquals,
    /// For substring-match fields (the inverse of `substring`).
    NotSubstring,
}

impl NegationOp {
    /// Returns the wire-form operator string Bugzilla expects in
    /// boolean-chart `oN` parameters.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotEquals => "notequals",
            Self::NotSubstring => "notsubstring",
        }
    }
}

/// Maps a filterable field across all naming contexts.
#[non_exhaustive]
pub struct FieldMapping {
    /// Name on `SearchParams` / `SavedQuery` (e.g. "status").
    /// Also used as the REST API query parameter.
    pub struct_field: &'static str,
    /// `buglist.cgi` URL parameter name (e.g. `bug_status`).
    pub url_param: &'static str,
    /// Bugzilla internal name for boolean charts (e.g. `bug_status`).
    pub internal_name: &'static str,
    /// Boolean-chart operator used when a value is negated (`!`-prefix).
    pub negation_operator: NegationOp,
}

/// Canonical field-mapping table for the 15 multi-value filter fields.
pub const FIELD_MAPPINGS: &[FieldMapping] = &[
    FieldMapping {
        struct_field: "product",
        url_param: "product",
        internal_name: "product",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "component",
        url_param: "component",
        internal_name: "component",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "status",
        url_param: "bug_status",
        internal_name: "bug_status",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "assigned_to",
        url_param: "assigned_to",
        internal_name: "assigned_to",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "creator",
        url_param: "reporter",
        internal_name: "reporter",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "priority",
        url_param: "priority",
        internal_name: "priority",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "severity",
        url_param: "bug_severity",
        internal_name: "bug_severity",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "whiteboard",
        url_param: "status_whiteboard",
        internal_name: "status_whiteboard",
        negation_operator: NegationOp::NotSubstring,
    },
    FieldMapping {
        struct_field: "target_milestone",
        url_param: "target_milestone",
        internal_name: "target_milestone",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "version",
        url_param: "version",
        internal_name: "version",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "op_sys",
        url_param: "op_sys",
        internal_name: "op_sys",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "platform",
        url_param: "rep_platform",
        internal_name: "rep_platform",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "resolution",
        url_param: "resolution",
        internal_name: "resolution",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "qa_contact",
        url_param: "qa_contact",
        internal_name: "qa_contact",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        struct_field: "url",
        url_param: "bug_file_loc",
        internal_name: "bug_file_loc",
        negation_operator: NegationOp::NotSubstring,
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

/// Represents an incremental update to a string-typed list field
/// (keywords, cc, groups, `see_also`). Bugzilla accepts
/// `{ "add": [...], "remove": [...] }` for these fields.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct StringListUpdate {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

impl StringListUpdate {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// A comment to post atomically with a `Bug.update` call.
///
/// Serializes as `{"body": "...", "is_private": <bool>}`. Bugzilla's
/// REST `Bug.update` accepts this as a sub-object on the request,
/// which delivers the field changes and the comment in one
/// round-trip.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct CommentUpdate {
    pub body: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_private: bool,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateBugParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dupe_of: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_time: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reset_assigned_to: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reset_qa_contact: bool,
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
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub keywords: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub cc: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub groups: StringListUpdate,
    #[serde(skip_serializing_if = "StringListUpdate::is_empty")]
    pub see_also: StringListUpdate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<CommentUpdate>,
    /// Edit the privacy of comments that are already on the bug.
    /// Keys are comment IDs; values are `true` (mark private) or
    /// `false` (mark public). Used by `attachment upload
    /// --comment-private` to flip the privacy of the comment created
    /// by `Bug.add_attachment`.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub comment_is_private: std::collections::HashMap<u64, bool>,
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
#[non_exhaustive]
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
    /// Filter by Status Whiteboard substring. See `SearchParams::whiteboard`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub whiteboard: Vec<String>,
    /// Filter by Target Milestone (exact match, repeatable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_milestone: Vec<String>,
    /// Filter by Version (exact match, repeatable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub version: Vec<String>,
    /// Filter by OS (exact match, repeatable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub op_sys: Vec<String>,
    /// Filter by Platform / Hardware (exact match, repeatable). API param `platform`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platform: Vec<String>,
    /// Filter by Resolution (exact match, repeatable; empty matches open bugs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolution: Vec<String>,
    /// Filter by QA Contact login (exact match, repeatable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qa_contact: Vec<String>,
    /// Filter by URL field substring (repeatable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub url: Vec<String>,
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
            whiteboard: self.whiteboard,
            target_milestone: self.target_milestone,
            version: self.version,
            op_sys: self.op_sys,
            platform: self.platform,
            resolution: self.resolution,
            qa_contact: self.qa_contact,
            url: self.url,
            ..Default::default()
        }
    }

    /// Access a multi-value filter field mutably by its `struct_field` name.
    /// Maps `assigned_to` to `self.assignee` (TOML-friendly name).
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Vec<String>> {
        FilterField::from_struct_field(name).map(|field| self.get_filter_field_mut(field))
    }

    fn get_field(&self, name: &str) -> Option<&[String]> {
        FilterField::from_struct_field(name).map(|field| self.get_filter_field(field))
    }

    fn get_filter_field(&self, field: FilterField) -> &[String] {
        match field {
            FilterField::Product => &self.product,
            FilterField::Component => &self.component,
            FilterField::Status => &self.status,
            FilterField::AssignedTo => &self.assignee,
            FilterField::Creator => &self.creator,
            FilterField::Priority => &self.priority,
            FilterField::Severity => &self.severity,
            FilterField::Whiteboard => &self.whiteboard,
            FilterField::TargetMilestone => &self.target_milestone,
            FilterField::Version => &self.version,
            FilterField::OpSys => &self.op_sys,
            FilterField::Platform => &self.platform,
            FilterField::Resolution => &self.resolution,
            FilterField::QaContact => &self.qa_contact,
            FilterField::Url => &self.url,
        }
    }

    fn get_filter_field_mut(&mut self, field: FilterField) -> &mut Vec<String> {
        match field {
            FilterField::Product => &mut self.product,
            FilterField::Component => &mut self.component,
            FilterField::Status => &mut self.status,
            FilterField::AssignedTo => &mut self.assignee,
            FilterField::Creator => &mut self.creator,
            FilterField::Priority => &mut self.priority,
            FilterField::Severity => &mut self.severity,
            FilterField::Whiteboard => &mut self.whiteboard,
            FilterField::TargetMilestone => &mut self.target_milestone,
            FilterField::Version => &mut self.version,
            FilterField::OpSys => &mut self.op_sys,
            FilterField::Platform => &mut self.platform,
            FilterField::Resolution => &mut self.resolution,
            FilterField::QaContact => &mut self.qa_contact,
            FilterField::Url => &mut self.url,
        }
    }

    fn has_mapped_filters(&self) -> bool {
        FIELD_MAPPINGS.iter().any(|mapping| {
            self.get_field(mapping.struct_field)
                .is_some_and(|field| !field.is_empty())
        })
    }

    /// Returns true if the query has any meaningful filters set.
    pub fn has_filters(&self) -> bool {
        self.has_mapped_filters()
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
