use serde::{Deserialize, Serialize, Serializer};

use super::bug::{FilterField, SearchParams, FIELD_MAPPINGS};

/// The kind of saved query implied by the fields that will execute.
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

    fn get_field(&self, field: FilterField) -> &[String] {
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

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
