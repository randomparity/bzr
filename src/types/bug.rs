use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::common::{Flag, FlagUpdate};

const BUG_BUILT_IN_FIELD_COUNT: usize = 25;

fn is_custom_field_name(name: &str) -> bool {
    name.starts_with("cf_")
}

/// Typed key for the multi-value filter fields shared by `SearchParams` and
/// `SavedQuery`. Used as the accessor key on both and embedded in
/// [`FieldMapping`] so callers iterate `FIELD_MAPPINGS` without re-parsing
/// field-name strings.
#[derive(Clone, Copy)]
pub enum FilterField {
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

#[derive(Debug)]
#[non_exhaustive]
pub struct Bug {
    pub id: u64,
    pub summary: String,
    pub status: String,
    pub resolution: Option<String>,
    pub dupe_of: Option<u64>,
    pub deadline: Option<String>,
    pub product: Option<String>,
    pub component: Option<String>,
    pub version: Option<String>,
    pub assigned_to: Option<String>,
    pub priority: Option<String>,
    pub severity: Option<String>,
    pub creation_time: Option<String>,
    pub last_change_time: Option<String>,
    pub creator: Option<String>,
    pub url: Option<String>,
    pub whiteboard: Option<String>,
    pub keywords: Vec<String>,
    pub blocks: Vec<u64>,
    pub depends_on: Vec<u64>,
    pub cc: Vec<String>,
    pub op_sys: Option<String>,
    pub rep_platform: Option<String>,
    pub target_milestone: Option<String>,
    pub flags: Vec<Flag>,
    pub custom_fields: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct BugWire {
    id: u64,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    dupe_of: Option<u64>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    assigned_to: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    creation_time: Option<String>,
    #[serde(default)]
    last_change_time: Option<String>,
    #[serde(default)]
    creator: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    whiteboard: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    blocks: Vec<u64>,
    #[serde(default)]
    depends_on: Vec<u64>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    op_sys: Option<String>,
    #[serde(default)]
    rep_platform: Option<String>,
    #[serde(default)]
    target_milestone: Option<String>,
    #[serde(default)]
    flags: Vec<Flag>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<BugWire> for Bug {
    fn from(wire: BugWire) -> Self {
        Bug {
            id: wire.id,
            summary: wire.summary,
            status: wire.status,
            resolution: wire.resolution,
            dupe_of: wire.dupe_of,
            deadline: wire.deadline,
            product: wire.product,
            component: wire.component,
            version: wire.version,
            assigned_to: wire.assigned_to,
            priority: wire.priority,
            severity: wire.severity,
            creation_time: wire.creation_time,
            last_change_time: wire.last_change_time,
            creator: wire.creator,
            url: wire.url,
            whiteboard: wire.whiteboard,
            keywords: wire.keywords,
            blocks: wire.blocks,
            depends_on: wire.depends_on,
            cc: wire.cc,
            op_sys: wire.op_sys,
            rep_platform: wire.rep_platform,
            target_milestone: wire.target_milestone,
            flags: wire.flags,
            custom_fields: wire
                .extra
                .into_iter()
                .filter(|(name, _)| is_custom_field_name(name))
                .collect(),
        }
    }
}

impl<'de> Deserialize<'de> for Bug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BugWire::deserialize(deserializer).map(Self::from)
    }
}

impl Serialize for Bug {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let custom_field_count = self
            .custom_fields
            .keys()
            .filter(|name| is_custom_field_name(name))
            .count();
        let mut map =
            serializer.serialize_map(Some(BUG_BUILT_IN_FIELD_COUNT + custom_field_count))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("summary", &self.summary)?;
        map.serialize_entry("status", &self.status)?;
        map.serialize_entry("resolution", &self.resolution)?;
        map.serialize_entry("dupe_of", &self.dupe_of)?;
        map.serialize_entry("deadline", &self.deadline)?;
        map.serialize_entry("product", &self.product)?;
        map.serialize_entry("component", &self.component)?;
        map.serialize_entry("version", &self.version)?;
        map.serialize_entry("assigned_to", &self.assigned_to)?;
        map.serialize_entry("priority", &self.priority)?;
        map.serialize_entry("severity", &self.severity)?;
        map.serialize_entry("creation_time", &self.creation_time)?;
        map.serialize_entry("last_change_time", &self.last_change_time)?;
        map.serialize_entry("creator", &self.creator)?;
        map.serialize_entry("url", &self.url)?;
        map.serialize_entry("whiteboard", &self.whiteboard)?;
        map.serialize_entry("keywords", &self.keywords)?;
        map.serialize_entry("blocks", &self.blocks)?;
        map.serialize_entry("depends_on", &self.depends_on)?;
        map.serialize_entry("cc", &self.cc)?;
        map.serialize_entry("op_sys", &self.op_sys)?;
        map.serialize_entry("rep_platform", &self.rep_platform)?;
        map.serialize_entry("target_milestone", &self.target_milestone)?;
        map.serialize_entry("flags", &self.flags)?;
        for (name, value) in self
            .custom_fields
            .iter()
            .filter(|(name, _)| is_custom_field_name(name))
        {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
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
    /// Number of leading matches to skip (Bugzilla `offset`). Combined with
    /// `limit` for manual paging; `None` means offset 0.
    pub offset: Option<u32>,
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
    /// Bugzilla `order` clause (e.g. `last_change_time DESC, bug_id`).
    /// Built from `--sort`/`--order`; defaults to a stable `bug_id` so
    /// identical runs return rows in a deterministic order.
    pub order: Option<String>,
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

/// Map a [`FilterField`] to the matching multi-value `Vec<String>` on `$self`.
///
/// Shared by `SearchParams` and `SavedQuery`, which carry identical filter
/// fields except for the assignee column (`assigned_to` vs `assignee`) — the
/// caller passes its own field name as `$assignee`. Pass a trailing `mut` for
/// a mutable borrow.
macro_rules! filter_field_arm {
    ($self:ident, $field:ident, $assignee:ident $(, $mutability:tt)?) => {
        match $field {
            FilterField::Product => & $($mutability)? $self.product,
            FilterField::Component => & $($mutability)? $self.component,
            FilterField::Status => & $($mutability)? $self.status,
            FilterField::AssignedTo => & $($mutability)? $self.$assignee,
            FilterField::Creator => & $($mutability)? $self.creator,
            FilterField::Priority => & $($mutability)? $self.priority,
            FilterField::Severity => & $($mutability)? $self.severity,
            FilterField::Whiteboard => & $($mutability)? $self.whiteboard,
            FilterField::TargetMilestone => & $($mutability)? $self.target_milestone,
            FilterField::Version => & $($mutability)? $self.version,
            FilterField::OpSys => & $($mutability)? $self.op_sys,
            FilterField::Platform => & $($mutability)? $self.platform,
            FilterField::Resolution => & $($mutability)? $self.resolution,
            FilterField::QaContact => & $($mutability)? $self.qa_contact,
            FilterField::Url => & $($mutability)? $self.url,
        }
    };
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

    /// Access a multi-value filter field by its typed [`FilterField`] key.
    pub(crate) fn get_field(&self, field: FilterField) -> &[String] {
        filter_field_arm!(self, field, assigned_to)
    }

    /// Access a multi-value filter field mutably by its typed [`FilterField`] key.
    pub(crate) fn get_field_mut(&mut self, field: FilterField) -> &mut Vec<String> {
        filter_field_arm!(self, field, assigned_to, mut)
    }

    fn has_mapped_filters(&self) -> bool {
        FIELD_MAPPINGS
            .iter()
            .any(|mapping| !self.get_field(mapping.field).is_empty())
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
    /// Typed key identifying the multi-value field on `SearchParams` / `SavedQuery`.
    pub field: FilterField,
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
        field: FilterField::Product,
        struct_field: "product",
        url_param: "product",
        internal_name: "product",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Component,
        struct_field: "component",
        url_param: "component",
        internal_name: "component",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Status,
        struct_field: "status",
        url_param: "bug_status",
        internal_name: "bug_status",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::AssignedTo,
        struct_field: "assigned_to",
        url_param: "assigned_to",
        internal_name: "assigned_to",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Creator,
        struct_field: "creator",
        url_param: "reporter",
        internal_name: "reporter",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Priority,
        struct_field: "priority",
        url_param: "priority",
        internal_name: "priority",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Severity,
        struct_field: "severity",
        url_param: "bug_severity",
        internal_name: "bug_severity",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Whiteboard,
        struct_field: "whiteboard",
        url_param: "status_whiteboard",
        internal_name: "status_whiteboard",
        negation_operator: NegationOp::NotSubstring,
    },
    FieldMapping {
        field: FilterField::TargetMilestone,
        struct_field: "target_milestone",
        url_param: "target_milestone",
        internal_name: "target_milestone",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Version,
        struct_field: "version",
        url_param: "version",
        internal_name: "version",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::OpSys,
        struct_field: "op_sys",
        url_param: "op_sys",
        internal_name: "op_sys",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Platform,
        struct_field: "platform",
        url_param: "rep_platform",
        internal_name: "rep_platform",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Resolution,
        struct_field: "resolution",
        url_param: "resolution",
        internal_name: "resolution",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::QaContact,
        struct_field: "qa_contact",
        url_param: "qa_contact",
        internal_name: "qa_contact",
        negation_operator: NegationOp::NotEquals,
    },
    FieldMapping {
        field: FilterField::Url,
        struct_field: "url",
        url_param: "bug_file_loc",
        internal_name: "bug_file_loc",
        negation_operator: NegationOp::NotSubstring,
    },
];

#[derive(Debug, Default, Serialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
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

impl UpdateBugParams {
    /// True when no field would be sent to the server — i.e. the params
    /// serialize to an empty JSON object. Every field declares
    /// `skip_serializing_if`, so serialization is the single source of
    /// truth for "no changes requested" and stays correct as fields are
    /// added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().map(serde_json::Map::is_empty))
            .unwrap_or(false)
    }
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

/// The kind of saved query — determines which fields are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Deserialize, Default)]
#[non_exhaustive]
pub struct SavedQuery {
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
    /// Persisted Bugzilla `order` clause (from `query save --sort/--order`).
    /// Overridden per-run by `query run --sort`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

#[derive(Serialize)]
struct SavedQueryWire<'a> {
    kind: QueryKind,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    product: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    component: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    status: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    assignee: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    creator: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    priority: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    severity: &'a Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quicksearch: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: &'a Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_fields: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server: &'a Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    raw_params: &'a Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creation_time: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_change_time: &'a Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    whiteboard: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    target_milestone: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    version: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    op_sys: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    platform: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    resolution: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    qa_contact: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    url: &'a Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: &'a Option<String>,
}

impl Serialize for SavedQuery {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SavedQueryWire {
            kind: self.kind(),
            product: &self.product,
            component: &self.component,
            status: &self.status,
            assignee: &self.assignee,
            creator: &self.creator,
            priority: &self.priority,
            severity: &self.severity,
            quicksearch: &self.quicksearch,
            limit: &self.limit,
            fields: &self.fields,
            exclude_fields: &self.exclude_fields,
            source_url: &self.source_url,
            server: &self.server,
            raw_params: &self.raw_params,
            creation_time: &self.creation_time,
            last_change_time: &self.last_change_time,
            whiteboard: &self.whiteboard,
            target_milestone: &self.target_milestone,
            version: &self.version,
            op_sys: &self.op_sys,
            platform: &self.platform,
            resolution: &self.resolution,
            qa_contact: &self.qa_contact,
            url: &self.url,
            order: &self.order,
        }
        .serialize(serializer)
    }
}

impl SavedQuery {
    /// Return the query mode implied by the fields that will execute.
    pub fn kind(&self) -> QueryKind {
        if self.source_url.is_some() || !self.raw_params.is_empty() {
            QueryKind::Url
        } else if self.quicksearch.is_some() {
            QueryKind::Search
        } else {
            QueryKind::List
        }
    }

    /// Convert this saved query into `SearchParams` by cloning.
    pub fn to_search_params(&self) -> SearchParams {
        self.clone().into_search_params()
    }

    /// Convert this saved query into `SearchParams`, consuming `self`.
    pub fn into_search_params(mut self) -> SearchParams {
        // The 15 multi-value filter fields are moved through the shared
        // [`FIELD_MAPPINGS`] table so a new filter only needs a table row, not
        // a hand-written assignment here. Each side's `get_field_mut` resolves
        // the `assigned_to`/`assignee` column divergence for its own struct.
        let mut params = SearchParams::default();
        for mapping in FIELD_MAPPINGS {
            *params.get_field_mut(mapping.field) =
                std::mem::take(self.get_field_mut(mapping.field));
        }
        SearchParams {
            quicksearch: self.quicksearch,
            limit: self.limit,
            include_fields: self.fields,
            exclude_fields: self.exclude_fields,
            raw_params: self.raw_params,
            creation_time: self.creation_time,
            last_change_time: self.last_change_time,
            order: self.order,
            ..params
        }
    }

    /// Access a multi-value filter field mutably by its typed [`FilterField`] key.
    /// Maps `assigned_to` to `self.assignee` (TOML-friendly name).
    pub(crate) fn get_field_mut(&mut self, field: FilterField) -> &mut Vec<String> {
        filter_field_arm!(self, field, assignee, mut)
    }

    fn get_field(&self, field: FilterField) -> &[String] {
        filter_field_arm!(self, field, assignee)
    }

    fn has_mapped_filters(&self) -> bool {
        FIELD_MAPPINGS
            .iter()
            .any(|mapping| !self.get_field(mapping.field).is_empty())
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
