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
    fn saved_query_has_filters_false_empty() {
        let query = SavedQuery::default();
        assert!(!query.has_filters());
    }

    fn sample_raw_params() -> Vec<(String, String)> {
        vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
        ]
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
            raw_params: sample_raw_params(),
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
    fn field_mappings_covers_all_search_params_vec_fields() {
        let params = SearchParams::default();
        for mapping in FIELD_MAPPINGS {
            let field = params.get_field(mapping.struct_field);
            assert!(
                field.is_empty(),
                "default field should be empty: {}",
                mapping.struct_field
            );
        }
    }

    #[test]
    fn field_mappings_has_expected_count() {
        assert_eq!(FIELD_MAPPINGS.len(), 7);
    }

    #[test]
    fn field_mappings_url_param_lookup() {
        let status = FIELD_MAPPINGS.iter().find(|m| m.url_param == "bug_status");
        assert!(status.is_some());
        assert_eq!(status.unwrap().struct_field, "status");
        assert_eq!(status.unwrap().internal_name, "bug_status");
    }

    #[test]
    fn field_mappings_internal_name_for_creator() {
        let creator = FIELD_MAPPINGS.iter().find(|m| m.struct_field == "creator");
        assert!(creator.is_some());
        assert_eq!(creator.unwrap().internal_name, "reporter");
    }

    #[test]
    fn search_params_get_field_returns_correct_data() {
        let params = SearchParams {
            product: vec!["Firefox".into()],
            status: vec!["NEW".into(), "ASSIGNED".into()],
            ..Default::default()
        };
        assert_eq!(params.get_field("product"), &["Firefox"]);
        assert_eq!(params.get_field("status"), &["NEW", "ASSIGNED"]);
        assert!(params.get_field("creator").is_empty());
    }

    #[test]
    #[should_panic(expected = "unknown field")]
    fn search_params_get_field_panics_on_unknown() {
        let params = SearchParams::default();
        params.get_field("nonexistent");
    }

    #[test]
    fn saved_query_get_field_mut_returns_correct_fields() {
        let mut query = SavedQuery::default();
        query
            .get_field_mut("assigned_to")
            .unwrap()
            .push("dev@example.com".into());
        assert_eq!(query.assignee, vec!["dev@example.com"]);

        query.get_field_mut("status").unwrap().push("NEW".into());
        assert_eq!(query.status, vec!["NEW"]);
    }

    #[test]
    fn saved_query_get_field_mut_returns_none_for_unknown() {
        let mut query = SavedQuery::default();
        assert!(query.get_field_mut("nonexistent").is_none());
    }

    #[test]
    fn search_params_has_filters_for_each_individual_field() {
        type Setter = fn(&mut SearchParams);
        let cases: &[(&str, Setter)] = &[
            ("product", |p| p.product.push("X".into())),
            ("component", |p| p.component.push("X".into())),
            ("status", |p| p.status.push("X".into())),
            ("assigned_to", |p| p.assigned_to.push("X".into())),
            ("creator", |p| p.creator.push("X".into())),
            ("priority", |p| p.priority.push("X".into())),
            ("severity", |p| p.severity.push("X".into())),
            ("cc", |p| p.cc = Some("X".into())),
            ("alias", |p| p.alias = Some("X".into())),
            ("id", |p| p.id = vec![1]),
            ("summary", |p| p.summary = Some("X".into())),
            ("quicksearch", |p| p.quicksearch = Some("X".into())),
            ("raw_params", |p| {
                p.raw_params = vec![("f1".into(), "X".into())];
            }),
        ];
        for (name, setter) in cases {
            let mut p = SearchParams::default();
            setter(&mut p);
            assert!(
                p.has_filters(),
                "field `{name}` alone should make has_filters() return true"
            );
        }
    }

    #[test]
    fn saved_query_has_filters_for_each_individual_field() {
        type Setter = fn(&mut SavedQuery);
        let cases: &[(&str, Setter)] = &[
            ("product", |q| q.product.push("X".into())),
            ("component", |q| q.component.push("X".into())),
            ("status", |q| q.status.push("X".into())),
            ("assignee", |q| q.assignee.push("X".into())),
            ("creator", |q| q.creator.push("X".into())),
            ("priority", |q| q.priority.push("X".into())),
            ("severity", |q| q.severity.push("X".into())),
            ("quicksearch", |q| q.quicksearch = Some("X".into())),
            ("raw_params", |q| {
                q.raw_params = vec![("f1".into(), "X".into())];
            }),
        ];
        for (name, setter) in cases {
            let mut q = SavedQuery::default();
            setter(&mut q);
            assert!(
                q.has_filters(),
                "field `{name}` alone should make has_filters() return true"
            );
        }
    }

    #[test]
    fn id_list_update_is_empty_when_both_empty() {
        let upd = IdListUpdate {
            add: vec![],
            remove: vec![],
        };
        assert!(upd.is_empty());
    }

    #[test]
    fn id_list_update_not_empty_when_only_add() {
        let upd = IdListUpdate {
            add: vec![1],
            remove: vec![],
        };
        assert!(!upd.is_empty());
    }

    #[test]
    fn id_list_update_not_empty_when_only_remove() {
        let upd = IdListUpdate {
            add: vec![],
            remove: vec![2],
        };
        assert!(!upd.is_empty());
    }

    #[test]
    fn into_search_params_moves_fields() {
        let query = SavedQuery {
            kind: QueryKind::List,
            product: vec!["Firefox".into()],
            component: vec!["General".into()],
            status: vec!["NEW".into()],
            assignee: vec!["dev@example.com".into()],
            creator: vec!["reporter@example.com".into()],
            priority: vec!["P1".into()],
            severity: vec!["critical".into()],
            quicksearch: Some("crash".into()),
            limit: Some(25),
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
            raw_params: vec![("f1".into(), "qa_contact".into())],
            ..Default::default()
        };
        let params = query.into_search_params();
        assert_eq!(params.product, vec!["Firefox"]);
        assert_eq!(params.component, vec!["General"]);
        assert_eq!(params.status, vec!["NEW"]);
        assert_eq!(params.assigned_to, vec!["dev@example.com"]);
        assert_eq!(params.creator, vec!["reporter@example.com"]);
        assert_eq!(params.priority, vec!["P1"]);
        assert_eq!(params.severity, vec!["critical"]);
        assert_eq!(params.quicksearch, Some("crash".into()));
        assert_eq!(params.limit, Some(25));
        assert_eq!(params.include_fields, Some("id,summary".into()));
        assert_eq!(params.exclude_fields, Some("comments".into()));
        assert_eq!(params.raw_params, vec![("f1".into(), "qa_contact".into())]);
    }
}
