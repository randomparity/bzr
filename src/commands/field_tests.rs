#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{FieldAction, ProjectionArgs};
use crate::test_helpers::setup_isolated_env;
use crate::types::OutputFormat;

fn list_with(name: &str, projection: ProjectionArgs) -> FieldAction {
    FieldAction::List {
        name: Some(name.to_string()),
        projection,
    }
}

async fn mount_status_values(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW", "sort_key": 0, "is_active": true},
                    {"name": "ASSIGNED", "sort_key": 1, "is_active": true}
                ]
            }]
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn field_list_returns_values() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW"},
                    {"name": "ASSIGNED"},
                    {"name": "RESOLVED"}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let action = FieldAction::List {
        name: Some("status".to_string()),
        projection: ProjectionArgs::default(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert!(parsed.as_array().unwrap().len() >= 3);
    assert_eq!(parsed[0]["name"], "NEW");
}

#[tokio::test]
async fn field_aliases_succeeds_without_server() {
    // No ENV_LOCK: `FieldAction::Aliases` returns the static FIELD_ALIASES list
    // and never loads config or reads the environment.
    let action = FieldAction::Aliases;
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["alias"], "file_loc");
    assert_eq!(arr[0]["api_name"], "bug_file_loc");
}

#[tokio::test]
async fn field_list_table_format_with_empty_values_prints_no_values_message() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "bug_status", "values": []}]
        })))
        .mount(&mock)
        .await;
    let action = FieldAction::List {
        name: Some("status".to_string()),
        projection: ProjectionArgs::default(),
    };
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
    assert!(result.is_ok());
    assert!(
        output.contains("No values for field"),
        "expected 'No values' message, got: {output:?}"
    );
}

#[tokio::test]
async fn field_list_json_format_with_empty_values_emits_empty_array() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "bug_status", "values": []}]
        })))
        .mount(&mock)
        .await;
    let action = FieldAction::List {
        name: Some("status".to_string()),
        projection: ProjectionArgs::default(),
    };
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __io_a4.writers(),
    )
    .await;
    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok());
    assert!(
        !output.contains("No values for field"),
        "JSON format must not emit the table-style 'No values' message; got: {output:?}"
    );
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert!(parsed.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn field_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = FieldAction::List {
        name: Some("status".to_string()),
        projection: ProjectionArgs::default(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn field_list_json_fields_projects_to_named_keys() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    mount_status_values(&mock).await;

    let action = list_with(
        "status",
        ProjectionArgs {
            fields: Some("name".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed[0]["name"], "NEW");
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn field_list_ndjson_fields_projects_each_line() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    mount_status_values(&mock).await;

    let action = list_with(
        "status",
        ProjectionArgs {
            fields: Some("name".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            OutputFormat::Ndjson,
            None,
        )
        .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(
        io.out_str().trim(),
        "{\"name\":\"NEW\"}\n{\"name\":\"ASSIGNED\"}"
    );
}

#[tokio::test]
async fn field_list_json_unknown_field_exits_7() {
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let action = list_with(
        "status",
        ProjectionArgs {
            fields: Some("nam".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn field_list_table_fields_is_noop_with_warning() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    mount_status_values(&mock).await;

    let action = list_with(
        "status",
        ProjectionArgs {
            fields: Some("name".into()),
            exclude_fields: None,
        },
    );
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(io.out_str().contains("NEW"));
    assert!(io
        .err_str()
        .contains("--fields/--exclude-fields only affect"));
}

async fn mount_catalogue_names(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "status_whiteboard"}, {"name": "keywords"}]
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn field_list_no_argument_lists_names() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    mount_catalogue_names(&mock).await;

    let action = FieldAction::List {
        name: None,
        projection: ProjectionArgs::default(),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "no-argument field list: {result:?}");
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    let rows = parsed.as_array().unwrap();
    let source_of = |name: &str| {
        rows.iter()
            .find(|row| row["name"] == name)
            .map(|row| row["source"].as_str().unwrap().to_string())
    };
    // Catalogue-only, bzr-only, and overlapping, in one assertion set: each
    // fails on a different half of the union going missing.
    assert_eq!(source_of("status_whiteboard").as_deref(), Some("server"));
    assert_eq!(source_of("whiteboard").as_deref(), Some("bzr"));
    assert_eq!(source_of("keywords").as_deref(), Some("both"));
}

/// `sort_key` is a valid key of the *named* form and an invalid key of this
/// one, so this fails if the handler validates against `FIELD_VALUE_FIELDS` by
/// mistake. A nonsense token would be rejected either way and prove nothing.
#[tokio::test]
async fn field_list_no_argument_rejects_unknown_projection() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;
    mount_catalogue_names(&mock).await;

    let action = FieldAction::List {
        name: None,
        projection: ProjectionArgs {
            fields: Some("sort_key".to_string()),
            ..ProjectionArgs::default()
        },
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut io.writers(),
    )
    .await;
    // `sort_key` is a FieldValue key, not a FieldName key.
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
}
