/// Typed key for the multi-value filter fields shared by `SearchParams` and
/// saved queries. Used as the accessor key on both and embedded in
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
    /// record field is `platform`); we match the search-API
    /// name here.
    pub platform: Vec<String>,
    /// Filter by Resolution (repeatable). Exact match. Empty
    /// resolution matches open bugs.
    pub resolution: Vec<String>,
    /// Filter by QA Contact login substring (repeatable). Negated values
    /// use the role-substring complement.
    pub qa_contact: Vec<String>,
    /// Filter by URL field substring (repeatable). Negated values
    /// use `notsubstring`.
    pub url: Vec<String>,
    /// Bugzilla `order` clause (e.g. `last_change_time DESC, bug_id`).
    /// Built from `--sort`/`--order`; defaults to a stable `bug_id` so
    /// identical runs return rows in a deterministic order.
    pub order: Option<String>,
}

/// Optional per-invocation overrides applied to a `SearchParams`.
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
/// Shared by `SearchParams` and saved queries, which carry identical filter
/// fields except for the assignee column (`assigned_to` vs `assignee`) - the
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
    /// excluded - they control pagination and field selection, not bug filtering.
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
    /// authoritative - retrying via XML-RPC will return the same set
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

    /// Return the first negated role filter whose remainder contains no
    /// Bugzilla search word. Bugzilla's role complement operator requires at
    /// least one whitespace/comma-delimited word.
    pub(crate) fn invalid_role_negation(&self) -> Option<(&'static str, &str)> {
        [
            ("--assignee", self.assigned_to.as_slice()),
            ("--creator", self.creator.as_slice()),
            ("--qa-contact", self.qa_contact.as_slice()),
        ]
        .into_iter()
        .find_map(|(flag, values)| {
            values.iter().find_map(|value| {
                let remainder = value.strip_prefix('!')?;
                let has_word = remainder
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .any(|word| !word.is_empty());
                (!has_word).then_some((flag, value.as_str()))
            })
        })
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
    /// For Bugzilla role fields (the inverse of `anywordssubstr`).
    NoWordsSubstring,
}

impl NegationOp {
    /// Returns the wire-form operator string Bugzilla expects in
    /// boolean-chart `oN` parameters.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotEquals => "notequals",
            Self::NotSubstring => "notsubstring",
            Self::NoWordsSubstring => "nowordssubstr",
        }
    }
}

/// Maps a filterable field across all naming contexts.
#[non_exhaustive]
pub struct FieldMapping {
    /// Typed key identifying the multi-value field on `SearchParams` / saved query.
    pub field: FilterField,
    /// Name on `SearchParams` / saved query (e.g. "status").
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
        negation_operator: NegationOp::NoWordsSubstring,
    },
    FieldMapping {
        field: FilterField::Creator,
        struct_field: "creator",
        url_param: "reporter",
        internal_name: "reporter",
        negation_operator: NegationOp::NoWordsSubstring,
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
        url_param: "platform",
        internal_name: "platform",
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
        negation_operator: NegationOp::NoWordsSubstring,
    },
    FieldMapping {
        field: FilterField::Url,
        struct_field: "url",
        url_param: "bug_file_loc",
        internal_name: "bug_file_loc",
        negation_operator: NegationOp::NotSubstring,
    },
];

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
