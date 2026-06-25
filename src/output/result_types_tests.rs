#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn action_result_created_json_shape() {
    let result = ActionResult::created(42, ResourceKind::Bug);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["id"], 42);
    assert_eq!(json["resource"], "bug");
    assert_eq!(json["action"], "created");
    assert!(json.get("name").is_none());
}

#[test]
fn action_result_created_named_includes_name() {
    let result = ActionResult::created_named(1, "widget", ResourceKind::Component);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["name"], "widget");
    assert_eq!(json["resource"], "component");
}

#[test]
fn upload_result_json_shape() {
    let result = UploadResult::new(10, 42, 1024);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["id"], 10);
    assert_eq!(json["bug_id"], 42);
    assert_eq!(json["size"], 1024);
    assert_eq!(json["resource"], "attachment");
    assert_eq!(json["action"], "created");
}

#[test]
fn download_result_json_shape() {
    let result = DownloadResult::new(5, "patch.diff", 512);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["id"], 5);
    assert_eq!(json["file"], "patch.diff");
    assert_eq!(json["resource"], "attachment");
    assert_eq!(json["action"], "downloaded");
}

#[test]
fn membership_result_added_json_shape() {
    let result = MembershipResult::added("alice", "admin");
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["user"], "alice");
    assert_eq!(json["group"], "admin");
    assert_eq!(json["action"], "added");
}

#[test]
fn membership_result_removed_json_shape() {
    let result = MembershipResult::removed("alice", "admin");
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["user"], "alice");
    assert_eq!(json["group"], "admin");
    assert_eq!(json["action"], "removed");
}

#[test]
fn tag_result_json_shape() {
    let result = TagResult::updated(7, vec!["important".into()]);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["comment_id"], 7);
    assert_eq!(json["tags"][0], "important");
    assert_eq!(json["action"], "updated");
}

#[test]
fn config_result_skip_none_fields() {
    let result = ConfigResult::default_set("prod", "/etc/bzr/config.toml");
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert!(json.get("url").is_none());
    assert!(json.get("is_default").is_none());
    assert_eq!(json["resource"], "server");
}

#[test]
fn config_result_configured_created_json_shape() {
    let result = ConfigResult::configured(
        "prod",
        "https://bugzilla.example",
        true,
        "/etc/bzr/config.toml",
        false,
    );
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["name"], "prod");
    assert_eq!(json["url"], "https://bugzilla.example");
    assert_eq!(json["is_default"], true);
    assert_eq!(json["action"], "created");
}

#[test]
fn config_result_configured_updated_json_shape() {
    let result = ConfigResult::configured(
        "prod",
        "https://bugzilla.example",
        false,
        "/etc/bzr/config.toml",
        true,
    );
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["is_default"], false);
    assert_eq!(json["action"], "updated");
}

#[test]
fn search_result_json_shape() {
    let result = SearchResult::new(vec!["foo".into(), "bar".into()]);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["items"][0], "foo");
    assert_eq!(json["items"][1], "bar");
}

#[test]
fn batch_result_json_shape() {
    let result = BatchResult::new(
        vec![1, 2],
        vec![BatchFailure {
            id: 3,
            error: "permission denied".into(),
        }],
    );
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["resource"], "bug");
    assert_eq!(json["action"], "updated");
    assert_eq!(json["succeeded"][0], 1);
    assert_eq!(json["failed"][0]["id"], 3);
    assert_eq!(json["failed"][0]["error"], "permission denied");
}

#[test]
fn action_result_updated_json_shape() {
    let result = ActionResult::updated(42, ResourceKind::User);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["id"], 42);
    assert_eq!(json["resource"], "user");
    assert_eq!(json["action"], "updated");
    assert!(json.get("name").is_none());
}

#[test]
fn action_result_updated_named_with_id_json_shape() {
    let result = ActionResult::updated_named(Some(9), "widget", ResourceKind::Component);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert_eq!(json["id"], 9);
    assert_eq!(json["name"], "widget");
    assert_eq!(json["resource"], "component");
    assert_eq!(json["action"], "updated");
}

#[test]
fn action_result_updated_named_without_id_skips_id() {
    let result = ActionResult::updated_named(None, "widget", ResourceKind::Component);
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    assert!(json.get("id").is_none());
    assert_eq!(json["name"], "widget");
    assert_eq!(json["action"], "updated");
}

#[test]
fn write_result_uses_pretty_json() {
    let result = ActionResult::created(1, ResourceKind::Bug);
    let mut buf = Vec::new();
    write_result(&result, "human", OutputFormat::Json, &mut buf);
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains('\n'), "expected pretty-printed JSON");
    assert!(s.contains("\"id\": 1"));
    assert!(s.contains("\"resource\": \"bug\""));
}

#[test]
fn write_result_table_emits_human_message() {
    let result = ActionResult::created(1, ResourceKind::Bug);
    let mut buf = Vec::new();
    write_result(&result, "Created bug #1", OutputFormat::Table, &mut buf);
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "Created bug #1\n");
}

#[test]
fn multi_bug_view_result_serializes_with_empty_failed() {
    let result = MultiBugViewResult {
        bugs: vec![],
        failed: vec![],
    };
    let v = serde_json::to_value(&result).unwrap();
    assert!(v.get("bugs").unwrap().is_array());
    assert!(v.get("failed").unwrap().is_array());
    assert_eq!(v["bugs"].as_array().unwrap().len(), 0);
    assert_eq!(v["failed"].as_array().unwrap().len(), 0);
}

#[test]
fn multi_bug_view_result_serializes_with_failure_id_as_string() {
    let result = MultiBugViewResult {
        bugs: vec![],
        failed: vec![BugViewFailure {
            id: "my-alias".into(),
            error: "bug not found: my-alias".into(),
        }],
    };
    let v = serde_json::to_value(&result).unwrap();
    let failed = &v["failed"][0];
    assert_eq!(failed["id"], "my-alias");
    assert_eq!(failed["error"], "bug not found: my-alias");
}
