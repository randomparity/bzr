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
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct SearchParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip)]
    pub id: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quicksearch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_fields: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_fields: Option<String>,
}

impl SearchParams {
    /// Returns true if any filter fields are set (product, component, etc.).
    ///
    /// Used by hybrid mode to decide whether an empty REST result warrants
    /// an XML-RPC retry — only retries when filters are present, since a
    /// filterless empty result is legitimately empty.
    ///
    /// Note: `limit`, `include_fields`, and `exclude_fields` are intentionally
    /// excluded — they control pagination and field selection, not bug filtering.
    pub fn has_filters(&self) -> bool {
        self.product.is_some()
            || self.component.is_some()
            || self.status.is_some()
            || self.assigned_to.is_some()
            || self.creator.is_some()
            || self.priority.is_some()
            || self.severity.is_some()
            || self.alias.is_some()
            || !self.id.is_empty()
            || self.summary.is_some()
            || self.quicksearch.is_some()
    }
}

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
}
