mod attachment;
mod bug;
mod comment;
mod common;
mod group;
mod product;
mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    Bug, CreateBugParams, FieldChange, FieldValue, HistoryEntry, SearchParams, StatusTransition,
    UpdateBugParams,
};
pub use comment::{Comment, UpdateCommentTagsParams};
pub use common::{
    ApiMode, AuthMethod, ExtensionInfo, FlagStatus, FlagUpdate, OutputFormat, ServerExtensions,
    ServerInfoResponse, ServerVersion,
};
pub use group::{CreateGroupParams, GroupInfo, GroupMember, UpdateGroupParams};
pub use product::{
    Classification, ClassificationProduct, Component, CreateComponentParams, CreateProductParams,
    Milestone, Product, ProductListType, UpdateComponentParams, UpdateProductParams, Version,
};
pub use user::{BugzillaUser, CreateUserParams, UpdateUserParams, UserGroup, WhoamiResponse};

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
    fn product_list_type_as_api_path() {
        assert_eq!(
            ProductListType::Accessible.as_api_path(),
            "product_accessible"
        );
        assert_eq!(
            ProductListType::Selectable.as_api_path(),
            "product_selectable"
        );
        assert_eq!(
            ProductListType::Enterable.as_api_path(),
            "product_enterable"
        );
    }

    #[test]
    fn product_list_type_default_is_accessible() {
        assert_eq!(ProductListType::default(), ProductListType::Accessible);
    }

    // FlagStatus tests

    #[test]
    fn flag_status_to_char() {
        assert_eq!(FlagStatus::Grant.to_char(), '+');
        assert_eq!(FlagStatus::Deny.to_char(), '-');
        assert_eq!(FlagStatus::Request.to_char(), '?');
        assert_eq!(FlagStatus::Clear.to_char(), 'X');
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
