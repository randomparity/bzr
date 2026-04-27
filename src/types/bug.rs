use serde::{Deserialize, Deserializer, Serialize};

use super::common::FlagUpdate;

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
}

impl SearchParams {
    /// Apply optional runtime overrides for limit, fields, and `exclude_fields`.
    pub fn apply_overrides(
        &mut self,
        limit: Option<u32>,
        fields: Option<&str>,
        exclude_fields: Option<&str>,
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
    }

    /// Returns true if any filter fields are set (product, component, etc.).
    ///
    /// Used by hybrid mode to decide whether an empty REST result warrants
    /// an XML-RPC retry — only retries when filters are present, since a
    /// filterless empty result is legitimately empty.
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

/// Maps `SearchParams` field names to Bugzilla internal field names
/// used in boolean chart `fN` parameters. Most are identical, but some
/// differ (e.g. `status` → `bug_status`, `creator` → `reporter`).
pub const BOOLEAN_CHART_FIELD_NAMES: &[(&str, &str)] = &[
    ("product", "product"),
    ("component", "component"),
    ("status", "bug_status"),
    ("assigned_to", "assigned_to"),
    ("creator", "reporter"),
    ("priority", "priority"),
    ("severity", "bug_severity"),
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
}

impl SavedQuery {
    /// Convert this saved query into `SearchParams` for the Bugzilla client.
    pub fn to_search_params(&self) -> SearchParams {
        SearchParams {
            product: self.product.clone(),
            component: self.component.clone(),
            status: self.status.clone(),
            assigned_to: self.assignee.clone(),
            creator: self.creator.clone(),
            priority: self.priority.clone(),
            severity: self.severity.clone(),
            quicksearch: self.quicksearch.clone(),
            limit: self.limit,
            include_fields: self.fields.clone(),
            exclude_fields: self.exclude_fields.clone(),
            raw_params: self.raw_params.clone(),
            ..Default::default()
        }
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
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bug_deserializes_minimal() {
        let json = r#"{"id": 42}"#;
        let bug: Bug = serde_json::from_str(json).unwrap();
        assert_eq!(bug.id, 42);
        assert!(bug.summary.is_empty());
        assert!(bug.keywords.is_empty());
    }

    #[test]
    fn bug_deserializes_full() {
        let json = r#"{"id": 1, "summary": "test bug", "status": "NEW", "product": "Core", "component": "General", "priority": "P1", "keywords": ["regression"]}"#;
        let bug: Bug = serde_json::from_str(json).unwrap();
        assert_eq!(bug.summary, "test bug");
        assert_eq!(bug.status, "NEW");
        assert_eq!(bug.product.as_deref(), Some("Core"));
        assert_eq!(bug.keywords, vec!["regression"]);
    }

    #[test]
    fn partition_filters_positive_only() {
        let vals: Vec<String> = vec!["NEW".into(), "ASSIGNED".into()];
        let (pos, neg) = partition_filters(&vals);
        assert_eq!(pos, vec!["NEW", "ASSIGNED"]);
        assert!(neg.is_empty());
    }

    #[test]
    fn partition_filters_negated_only() {
        let vals: Vec<String> = vec!["!CLOSED".into(), "!VERIFIED".into()];
        let (pos, neg) = partition_filters(&vals);
        assert!(pos.is_empty());
        assert_eq!(neg, vec!["CLOSED", "VERIFIED"]);
    }

    #[test]
    fn partition_filters_mixed() {
        let vals: Vec<String> = vec!["NEW".into(), "!CLOSED".into(), "OPEN".into()];
        let (pos, neg) = partition_filters(&vals);
        assert_eq!(pos, vec!["NEW", "OPEN"]);
        assert_eq!(neg, vec!["CLOSED"]);
    }

    #[test]
    fn partition_filters_empty() {
        let vals: Vec<String> = vec![];
        let (pos, neg) = partition_filters(&vals);
        assert!(pos.is_empty());
        assert!(neg.is_empty());
    }

    #[test]
    fn field_value_null_name_becomes_empty() {
        let json = r#"{"name": null, "sort_key": 0, "is_active": true}"#;
        let fv: FieldValue = serde_json::from_str(json).unwrap();
        assert!(fv.name.is_empty());
    }

    #[test]
    fn field_value_with_name() {
        let json = r#"{"name": "RESOLVED", "sort_key": 5, "is_active": true}"#;
        let fv: FieldValue = serde_json::from_str(json).unwrap();
        assert_eq!(fv.name, "RESOLVED");
        assert_eq!(fv.sort_key, 5);
        assert!(fv.is_active);
    }

    #[test]
    fn saved_query_list_roundtrips_json() {
        let query = SavedQuery {
            kind: QueryKind::List,
            product: vec!["Firefox".into()],
            component: vec![],
            status: vec!["NEW".into(), "ASSIGNED".into()],
            assignee: vec![],
            creator: vec![],
            priority: vec!["P1".into()],
            severity: vec![],
            quicksearch: None,
            limit: Some(25),
            fields: None,
            exclude_fields: None,
            source_url: None,
            server: None,
            raw_params: vec![],
        };
        let json = serde_json::to_string(&query).unwrap();
        let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.kind, QueryKind::List);
        assert_eq!(roundtripped.product, vec!["Firefox"]);
        assert_eq!(roundtripped.status, vec!["NEW", "ASSIGNED"]);
        assert_eq!(roundtripped.limit, Some(25));
    }

    #[test]
    fn saved_query_search_roundtrips_json() {
        let query = SavedQuery {
            kind: QueryKind::Search,
            quicksearch: Some("crash in tab".into()),
            limit: Some(10),
            ..SavedQuery::default()
        };
        let json = serde_json::to_string(&query).unwrap();
        let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.kind, QueryKind::Search);
        assert_eq!(roundtripped.quicksearch.as_deref(), Some("crash in tab"));
    }

    #[test]
    fn saved_query_to_search_params_list() {
        let query = SavedQuery {
            kind: QueryKind::List,
            product: vec!["Core".into()],
            status: vec!["NEW".into()],
            limit: Some(20),
            fields: Some("id,summary".into()),
            ..SavedQuery::default()
        };
        let params = query.to_search_params();
        assert_eq!(params.product, vec!["Core"]);
        assert_eq!(params.status, vec!["NEW"]);
        assert_eq!(params.limit, Some(20));
        assert_eq!(params.include_fields.as_deref(), Some("id,summary"));
        assert!(params.quicksearch.is_none());
    }

    #[test]
    fn saved_query_to_search_params_search() {
        let query = SavedQuery {
            kind: QueryKind::Search,
            quicksearch: Some("memory leak".into()),
            limit: Some(30),
            ..SavedQuery::default()
        };
        let params = query.to_search_params();
        assert_eq!(params.quicksearch.as_deref(), Some("memory leak"));
        assert_eq!(params.limit, Some(30));
        assert!(params.product.is_empty());
    }

    #[test]
    fn saved_query_has_filters_true() {
        let query = SavedQuery {
            kind: QueryKind::List,
            product: vec!["Firefox".into()],
            ..SavedQuery::default()
        };
        assert!(query.has_filters());
    }

    #[test]
    fn saved_query_has_filters_false_empty() {
        let query = SavedQuery::default();
        assert!(!query.has_filters());
    }

    #[test]
    fn saved_query_has_filters_search_only() {
        let query = SavedQuery {
            kind: QueryKind::Search,
            quicksearch: Some("crash".into()),
            ..SavedQuery::default()
        };
        assert!(query.has_filters());
    }

    #[test]
    fn query_kind_url_serializes() {
        let json = serde_json::to_string(&QueryKind::Url).unwrap();
        assert_eq!(json, r#""url""#);
    }

    #[test]
    fn query_kind_url_deserializes() {
        let kind: QueryKind = serde_json::from_str(r#""url""#).unwrap();
        assert_eq!(kind, QueryKind::Url);
    }

    #[test]
    fn saved_query_with_url_fields_roundtrips() {
        let query = SavedQuery {
            kind: QueryKind::Url,
            source_url: Some("https://bugzilla.example.com/buglist.cgi?product=Firefox".into()),
            server: Some("example".into()),
            raw_params: vec![
                ("f1".into(), "qa_contact".into()),
                ("o1".into(), "changedfrom".into()),
                ("v1".into(), "user@example.com".into()),
            ],
            product: vec!["Firefox".into()],
            ..SavedQuery::default()
        };
        let json = serde_json::to_string(&query).unwrap();
        let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.kind, QueryKind::Url);
        assert_eq!(
            roundtripped.source_url.as_deref(),
            Some("https://bugzilla.example.com/buglist.cgi?product=Firefox")
        );
        assert_eq!(roundtripped.server.as_deref(), Some("example"));
        assert_eq!(roundtripped.raw_params.len(), 3);
        assert_eq!(
            roundtripped.raw_params[0],
            ("f1".into(), "qa_contact".into())
        );
        assert_eq!(roundtripped.product, vec!["Firefox"]);
    }

    #[test]
    fn saved_query_without_url_fields_omits_them_in_json() {
        let query = SavedQuery {
            kind: QueryKind::List,
            product: vec!["Firefox".into()],
            ..SavedQuery::default()
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(!json.contains("source_url"));
        assert!(!json.contains("\"server\""));
        assert!(!json.contains("raw_params"));
    }

    #[test]
    fn saved_query_url_kind_to_search_params_includes_raw_params() {
        let query = SavedQuery {
            kind: QueryKind::Url,
            product: vec!["Firefox".into()],
            raw_params: vec![
                ("f1".into(), "qa_contact".into()),
                ("o1".into(), "changedfrom".into()),
            ],
            limit: Some(100),
            ..SavedQuery::default()
        };
        let params = query.to_search_params();
        assert_eq!(params.product, vec!["Firefox"]);
        assert_eq!(params.limit, Some(100));
        assert_eq!(params.raw_params.len(), 2);
        assert_eq!(params.raw_params[0], ("f1".into(), "qa_contact".into()));
    }

    #[test]
    fn saved_query_url_kind_has_filters_with_only_raw_params() {
        let query = SavedQuery {
            kind: QueryKind::Url,
            raw_params: vec![("f1".into(), "qa_contact".into())],
            ..SavedQuery::default()
        };
        assert!(query.has_filters());
    }

    #[test]
    fn search_params_has_filters_with_raw_params() {
        let params = SearchParams {
            raw_params: vec![("f1".into(), "qa_contact".into())],
            ..Default::default()
        };
        assert!(params.has_filters());
    }
}
