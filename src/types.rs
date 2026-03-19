use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProductListType {
    #[default]
    Accessible,
    Selectable,
    Enterable,
}

impl std::str::FromStr for ProductListType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "accessible" => Ok(Self::Accessible),
            "selectable" => Ok(Self::Selectable),
            "enterable" => Ok(Self::Enterable),
            other => Err(format!(
                "invalid product type '{other}': expected 'accessible', 'selectable', or 'enterable'"
            )),
        }
    }
}

impl ProductListType {
    pub fn as_endpoint(self) -> &'static str {
        match self {
            Self::Accessible => "product_accessible",
            Self::Selectable => "product_selectable",
            Self::Enterable => "product_enterable",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub is_private: bool,
}

#[derive(Debug, Default, Serialize)]
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
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
}

#[derive(Debug, Default, Serialize)]
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

// Product / field / user types

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub versions: Vec<Version>,
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub default_assignee: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Milestone {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldValue {
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub can_change_to: Option<Vec<StatusTransition>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusTransition {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BugzillaUser {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub groups: Vec<UserGroup>,
    #[serde(default)]
    pub can_login: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserGroup {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

// Whoami type

#[derive(Debug, Serialize, Deserialize)]
pub struct WhoamiResponse {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
}

// Server info types

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerVersion {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerExtensions {
    pub extensions: HashMap<String, ExtensionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionInfo {
    #[serde(default)]
    pub version: Option<String>,
}

// Flag types

/// Represents the four valid flag status values in Bugzilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagStatus {
    /// `+` — flag granted
    Grant,
    /// `-` — flag denied
    Deny,
    /// `?` — flag requested
    Request,
    /// `X` — flag cleared/removed
    Clear,
}

impl FlagStatus {
    pub fn as_char(self) -> char {
        match self {
            FlagStatus::Grant => '+',
            FlagStatus::Deny => '-',
            FlagStatus::Request => '?',
            FlagStatus::Clear => 'X',
        }
    }
}

impl std::fmt::Display for FlagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

impl Serialize for FlagStatus {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_char(self.as_char())
    }
}

impl<'de> Deserialize<'de> for FlagStatus {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "+" => Ok(FlagStatus::Grant),
            "-" => Ok(FlagStatus::Deny),
            "?" => Ok(FlagStatus::Request),
            "X" => Ok(FlagStatus::Clear),
            other => Err(serde::de::Error::custom(format!(
                "invalid flag status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagUpdate {
    pub name: String,
    pub status: FlagStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestee: Option<String>,
}

// Attachment params

pub struct UploadAttachmentParams {
    pub bug_id: u64,
    pub file_name: String,
    pub summary: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub flags: Vec<FlagUpdate>,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateAttachmentParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_obsolete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_patch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagUpdate>,
}

// Admin param types

#[derive(Debug, Serialize)]
pub struct CreateProductParams {
    pub name: String,
    pub description: String,
    pub version: String,
    pub is_open: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateProductParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_open: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateComponentParams {
    pub product: String,
    pub name: String,
    pub description: String,
    pub default_assignee: String,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateComponentParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_assignee: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateUserParams {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateUserParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_denied_text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateGroupParams {
    pub name: String,
    pub description: String,
    pub is_active: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateGroupParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

// Group info type (for group view)

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupInfo {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub membership: Vec<GroupMember>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

// Classification types

#[derive(Debug, Serialize, Deserialize)]
pub struct Classification {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub products: Vec<ClassificationProduct>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassificationProduct {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

// Bug history types

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub who: String,
    pub when: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize)]
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
pub struct Attachment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub is_obsolete: bool,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub data: Option<String>,
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ProductListType tests

    #[test]
    fn product_list_type_from_str_valid() {
        assert_eq!(
            "accessible".parse::<ProductListType>().unwrap(),
            ProductListType::Accessible
        );
        assert_eq!(
            "selectable".parse::<ProductListType>().unwrap(),
            ProductListType::Selectable
        );
        assert_eq!(
            "enterable".parse::<ProductListType>().unwrap(),
            ProductListType::Enterable
        );
    }

    #[test]
    fn product_list_type_from_str_invalid() {
        let err = "bogus".parse::<ProductListType>().unwrap_err();
        assert!(err.contains("invalid product type"));
    }

    #[test]
    fn product_list_type_as_endpoint() {
        assert_eq!(
            ProductListType::Accessible.as_endpoint(),
            "product_accessible"
        );
        assert_eq!(
            ProductListType::Selectable.as_endpoint(),
            "product_selectable"
        );
        assert_eq!(
            ProductListType::Enterable.as_endpoint(),
            "product_enterable"
        );
    }

    #[test]
    fn product_list_type_default_is_accessible() {
        assert_eq!(ProductListType::default(), ProductListType::Accessible);
    }

    // FlagStatus tests

    #[test]
    fn flag_status_as_char() {
        assert_eq!(FlagStatus::Grant.as_char(), '+');
        assert_eq!(FlagStatus::Deny.as_char(), '-');
        assert_eq!(FlagStatus::Request.as_char(), '?');
        assert_eq!(FlagStatus::Clear.as_char(), 'X');
    }

    #[test]
    fn flag_status_display() {
        assert_eq!(FlagStatus::Grant.to_string(), "+");
        assert_eq!(FlagStatus::Deny.to_string(), "-");
        assert_eq!(FlagStatus::Request.to_string(), "?");
        assert_eq!(FlagStatus::Clear.to_string(), "X");
    }

    #[test]
    fn flag_status_serialize_deserialize_roundtrip() {
        let update = FlagUpdate {
            name: "review".to_string(),
            status: FlagStatus::Grant,
            requestee: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains(r#""status":"+""#));

        let back: FlagUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status, FlagStatus::Grant);
        assert_eq!(back.name, "review");
    }

    #[test]
    fn flag_status_deserialize_all_variants() {
        for (ch, expected) in [
            ("+", FlagStatus::Grant),
            ("-", FlagStatus::Deny),
            ("?", FlagStatus::Request),
            ("X", FlagStatus::Clear),
        ] {
            let json = format!(r#"{{"name":"f","status":"{ch}"}}"#);
            let flag: FlagUpdate = serde_json::from_str(&json).unwrap();
            assert_eq!(flag.status, expected);
        }
    }

    #[test]
    fn flag_status_deserialize_invalid() {
        let json = r#"{"name":"f","status":"Z"}"#;
        let result = serde_json::from_str::<FlagUpdate>(json);
        assert!(result.is_err());
    }

    // SearchParams tests

    #[test]
    fn search_params_has_filters_empty() {
        let params = SearchParams::default();
        assert!(!params.has_filters());
    }

    #[test]
    fn search_params_has_filters_with_product() {
        let params = SearchParams {
            product: Some("TestProduct".into()),
            ..Default::default()
        };
        assert!(params.has_filters());
    }

    #[test]
    fn search_params_has_filters_with_ids() {
        let params = SearchParams {
            id: vec![1, 2, 3],
            ..Default::default()
        };
        assert!(params.has_filters());
    }

    #[test]
    fn search_params_has_filters_with_quicksearch() {
        let params = SearchParams {
            quicksearch: Some("crash".into()),
            ..Default::default()
        };
        assert!(params.has_filters());
    }

    // Bug deserialization tests

    #[test]
    fn bug_deserialize_minimal() {
        let json = r#"{"id": 42}"#;
        let bug: Bug = serde_json::from_str(json).unwrap();
        assert_eq!(bug.id, 42);
        assert_eq!(bug.summary, "");
        assert_eq!(bug.status, "");
        assert!(bug.resolution.is_none());
    }

    #[test]
    fn bug_deserialize_full() {
        let json = r#"{
            "id": 1,
            "summary": "Test bug",
            "status": "NEW",
            "resolution": "FIXED",
            "product": "TestProduct",
            "component": "General",
            "assigned_to": "user@example.com",
            "keywords": ["crash", "regression"],
            "blocks": [2, 3],
            "depends_on": [4]
        }"#;
        let bug: Bug = serde_json::from_str(json).unwrap();
        assert_eq!(bug.id, 1);
        assert_eq!(bug.summary, "Test bug");
        assert_eq!(bug.keywords, vec!["crash", "regression"]);
        assert_eq!(bug.blocks, vec![2, 3]);
    }

    // Comment deserialization

    #[test]
    fn comment_deserialize_minimal() {
        let json = r#"{"id": 100}"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, 100);
        assert_eq!(comment.text, "");
        assert!(!comment.is_private);
    }

    // Attachment deserialization

    #[test]
    fn attachment_deserialize_minimal() {
        let json = r#"{"id": 50}"#;
        let att: Attachment = serde_json::from_str(json).unwrap();
        assert_eq!(att.id, 50);
        assert_eq!(att.file_name, "");
        assert!(!att.is_obsolete);
    }

    // WhoamiResponse deserialization

    #[test]
    fn whoami_deserialize() {
        let json = r#"{"id": 1, "name": "admin@example.com", "real_name": "Admin"}"#;
        let whoami: WhoamiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(whoami.id, 1);
        assert_eq!(whoami.name, "admin@example.com");
        assert_eq!(whoami.real_name.as_deref(), Some("Admin"));
    }

    // UpdateBugParams serialization

    #[test]
    fn update_bug_params_skips_none_fields() {
        let params = UpdateBugParams {
            status: Some("RESOLVED".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["status"], "RESOLVED");
        assert!(json.get("resolution").is_none());
        assert!(json.get("flags").is_none());
    }

    // GroupInfo deserialization

    #[test]
    fn group_info_deserialize() {
        let json = r#"{
            "id": 10,
            "name": "admin",
            "description": "Administrators",
            "is_active": true,
            "membership": [{"id": 1, "name": "user@test.com"}]
        }"#;
        let group: GroupInfo = serde_json::from_str(json).unwrap();
        assert_eq!(group.id, 10);
        assert_eq!(group.name, "admin");
        assert!(group.is_active);
        assert_eq!(group.membership.len(), 1);
    }

    // Classification deserialization

    #[test]
    fn classification_deserialize() {
        let json = r#"{
            "id": 1,
            "name": "Unclassified",
            "description": "Default",
            "sort_key": 0,
            "products": [{"id": 1, "name": "TestProduct", "description": "Test"}]
        }"#;
        let cls: Classification = serde_json::from_str(json).unwrap();
        assert_eq!(cls.name, "Unclassified");
        assert_eq!(cls.products.len(), 1);
    }

    // HistoryEntry deserialization

    #[test]
    fn history_entry_deserialize() {
        let json = r#"{
            "who": "user@test.com",
            "when": "2025-01-01T00:00:00Z",
            "changes": [{"field_name": "status", "removed": "NEW", "added": "RESOLVED"}]
        }"#;
        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.who, "user@test.com");
        assert_eq!(entry.changes.len(), 1);
        assert_eq!(entry.changes[0].field_name, "status");
        assert_eq!(entry.changes[0].added, "RESOLVED");
    }
}
