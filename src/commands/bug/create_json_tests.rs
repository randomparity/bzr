#![expect(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::from_json::JsonOneOrMany;
use crate::error::BzrError;
use crate::types::{OutputFormat, ProgressFormat};

use super::JsonCreateBug;

fn sample_create_params() -> crate::types::bug::CreateBugParams {
    crate::types::bug::CreateBugParams {
        product: "P".into(),
        component: "C".into(),
        summary: "S".into(),
        version: "unspecified".into(),
        ..Default::default()
    }
}

/// Pair create params with an empty compound plan (no comment/attachments) for
/// the batch driver, whose signature now carries per-element plans.
fn plain(
    params: crate::types::bug::CreateBugParams,
) -> (
    crate::types::bug::CreateBugParams,
    crate::commands::bug::compound::CompoundPlan,
) {
    (
        params,
        crate::commands::bug::compound::CompoundPlan {
            comment: None,
            attachments: vec![],
        },
    )
}

#[tokio::test]
async fn batch_create_emits_batch_then_done() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;
    let prepared = vec![plain(sample_create_params()), plain(sample_create_params())];
    let ctx = CommandContext::new(None, OutputFormat::Json, None)
        .with_progress(Some(ProgressFormat::Ndjson));
    let mut io = crate::test_helpers::CapturedIo::new();
    super::create_batch_from_json(prepared, &ctx, &mut io.writers())
        .await
        .unwrap();
    assert_eq!(
        io.err_str().lines().collect::<Vec<_>>(),
        vec![
            "{\"event\":\"batch\",\"n\":1,\"total\":2,\"ok\":1,\"failed\":0}",
            "{\"event\":\"batch\",\"n\":2,\"total\":2,\"ok\":2,\"failed\":0}",
            "{\"event\":\"done\",\"fetched\":2}",
        ]
    );
}

#[tokio::test]
async fn batch_create_partial_failure_emits_no_done() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    // First POST succeeds (single use); the second falls through to a 500.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let prepared = vec![plain(sample_create_params()), plain(sample_create_params())];
    let ctx = CommandContext::new(None, OutputFormat::Json, None)
        .with_progress(Some(ProgressFormat::Ndjson));
    let mut io = crate::test_helpers::CapturedIo::new();
    let res = super::create_batch_from_json(prepared, &ctx, &mut io.writers()).await;
    assert!(res.is_err(), "partial failure exits non-zero");
    let err = io.err_str();
    assert!(
        err.contains("\"event\":\"batch\""),
        "per-item events still emit"
    );
    assert!(
        !err.contains("\"event\":\"done\""),
        "no done on partial failure"
    );
}

fn schema_value(name: &str) -> serde_json::Value {
    let (_, body) = crate::commands::schema::SCHEMAS
        .iter()
        .find(|(schema_name, _)| *schema_name == name)
        .unwrap_or_else(|| panic!("schema '{name}' is not registered"));
    serde_json::from_str(body).unwrap_or_else(|err| panic!("schema '{name}' is invalid: {err}"))
}

fn schema_object_properties(
    name: &str,
    def_name: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let pointer = format!("/$defs/{def_name}/properties");
    schema_value(name)
        .pointer(&pointer)
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("schema '{name}' missing {pointer}"))
        .clone()
}

fn parser_fields_from_unknown_error<T>() -> BTreeSet<String>
where
    T: serde::de::DeserializeOwned,
{
    let err = match serde_json::from_value::<T>(serde_json::json!({
        "__bzr_unknown_field__": true,
    })) {
        Ok(_) => panic!("unknown sentinel field unexpectedly parsed"),
        Err(err) => err.to_string(),
    };
    let expected = err
        .split_once("expected ")
        .unwrap_or_else(|| panic!("serde error did not list expected fields: {err}"))
        .1;
    expected
        .split('`')
        .skip(1)
        .step_by(2)
        .map(ToString::to_string)
        .collect()
}

#[test]
fn bug_create_input_schema_matches_parser_keys() {
    let schema_keys: BTreeSet<String> =
        schema_object_properties("bug-create-input", "bugCreateInputObject")
            .keys()
            .cloned()
            .collect();
    let parser_keys = parser_fields_from_unknown_error::<JsonCreateBug>();

    assert_eq!(schema_keys, parser_keys);
}

#[test]
fn bug_create_input_schema_examples_parse() {
    for (key, property_schema) in
        schema_object_properties("bug-create-input", "bugCreateInputObject")
    {
        let example = property_schema
            .get("examples")
            .and_then(serde_json::Value::as_array)
            .and_then(|examples| examples.first())
            .unwrap_or_else(|| panic!("bug-create-input property '{key}' missing example"))
            .clone();
        let mut object = serde_json::Map::new();
        object.insert(key.clone(), example);

        serde_json::from_value::<JsonCreateBug>(serde_json::Value::Object(object))
            .unwrap_or_else(|err| panic!("schema example for '{key}' did not parse: {err}"));
    }
}

#[test]
fn bug_create_input_schema_documents_array_form() {
    let schema = schema_value("bug-create-input");
    let one_of = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("bug-create-input schema missing oneOf"));
    let array_branch = one_of
        .iter()
        .find(|branch| branch.get("type").and_then(serde_json::Value::as_str) == Some("array"))
        .unwrap_or_else(|| panic!("bug-create-input schema does not document array input"));

    assert_eq!(
        array_branch
            .get("minItems")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn parse_one_or_many_rejects_non_object_scalar() {
    let err =
        crate::commands::runtime::from_json::parse_one_or_many::<JsonCreateBug>("42").unwrap_err();
    assert!(matches!(err, BzrError::InputValidation(_)));
}

#[test]
fn parse_one_or_many_rejects_malformed_json() {
    let err = crate::commands::runtime::from_json::parse_one_or_many::<JsonCreateBug>("{not json")
        .unwrap_err();
    match err {
        BzrError::InputValidation(msg) => assert!(msg.contains("invalid JSON"), "{msg}"),
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn parse_one_or_many_preserves_object_and_array_shapes() {
    assert!(matches!(
        crate::commands::runtime::from_json::parse_one_or_many::<JsonCreateBug>(
            r#"{"product":"P"}"#
        )
        .unwrap(),
        JsonOneOrMany::One(_)
    ));
    match crate::commands::runtime::from_json::parse_one_or_many::<JsonCreateBug>(
        r#"[{"product":"P"},{"product":"Q"}]"#,
    )
    .unwrap()
    {
        JsonOneOrMany::Many(v) => assert_eq!(v.len(), 2),
        JsonOneOrMany::One(_) => panic!("array should parse as Many"),
    }
    // A 1-element array stays Many, so output shape follows input shape.
    assert!(matches!(
        crate::commands::runtime::from_json::parse_one_or_many::<JsonCreateBug>(
            r#"[{"product":"P"}]"#
        )
        .unwrap(),
        JsonOneOrMany::Many(_)
    ));
}

// ── Compound --from-json (comment / attachments) ─────────────────────

#[test]
fn compound_keys_default_to_absent_and_yield_empty_plan() {
    let mut entry: JsonCreateBug = serde_json::from_value(serde_json::json!({
        "product": "P", "component": "C", "summary": "S"
    }))
    .unwrap();
    let plan = entry.take_plan().unwrap();
    assert!(plan.is_empty());
}

#[test]
fn compound_empty_comment_body_is_rejected() {
    let mut entry: JsonCreateBug = serde_json::from_value(serde_json::json!({
        "product": "P", "component": "C", "summary": "S",
        "comment": {"body": "   "}
    }))
    .unwrap();
    let err = entry.take_plan().unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(matches!(err, BzrError::InputValidation(_)));
}

#[test]
fn compound_comment_unknown_key_is_rejected() {
    let err = serde_json::from_value::<JsonCreateBug>(serde_json::json!({
        "product": "P", "component": "C", "summary": "S",
        "comment": {"body": "x", "bogus": true}
    }))
    .unwrap_err();
    assert!(err.to_string().contains("bogus"), "error was: {err}");
}

#[tokio::test]
async fn array_one_good_one_failing_comment_exits_11() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 10})))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 11})))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/11/comment"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let prepared = vec![
        plain(sample_create_params()),
        (
            sample_create_params(),
            crate::commands::bug::compound::CompoundPlan {
                comment: Some(crate::types::comment::AddCommentParams {
                    text: "note".into(),
                    is_private: false,
                }),
                attachments: vec![],
            },
        ),
    ];
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = super::create_batch_from_json(prepared, &ctx, &mut io.writers())
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 11);
    assert!(io.err_str().contains("11"), "stderr: {}", io.err_str());
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    let created: Vec<u64> = data["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(created, vec![10, 11]);
    assert_eq!(data["failed"][0]["index"], 1);
    assert_eq!(data["failed"][0]["bug_id"], 11);
    assert_eq!(data["failed"][0]["step"], "comment");
}

fn from_json_args(path: &str) -> crate::cli::CreateArgs {
    crate::cli::CreateArgs {
        from_json: Some(path.to_string()),
        template: None,
        product: None,
        component: None,
        summary: None,
        version: None,
        description: None,
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
        with_comment: None,
        with_comment_file: None,
        with_attachment: vec![],
        attachment_description: vec![],
        create_fields: crate::cli::CreateFieldArgs::default(),
    }
}

#[tokio::test]
async fn array_with_bad_attachment_file_creates_nothing() {
    let (_lock, mock, tmp) = crate::test_helpers::setup_test_env().await;
    // Any create POST is a failure: phase-1 validation must abort first.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 1})))
        .expect(0)
        .mount(&mock)
        .await;
    let payload = serde_json::json!([
        {"product": "P", "component": "C", "summary": "good"},
        {"product": "P", "component": "C", "summary": "bad",
         "attachments": [{"file": "/no/such/bzr-missing-attachment.log"}]}
    ])
    .to_string();
    let json_path = tmp.path().join("batch.json");
    std::fs::write(&json_path, payload).unwrap();
    let args = from_json_args(json_path.to_str().unwrap());
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = super::handle(&args, json_path.to_str().unwrap(), &ctx, &mut io.writers())
        .await
        .unwrap_err();
    // Unreadable attachment file surfaces as an I/O error (exit 6), consistent
    // with `attachment upload`; the key guarantee is that it aborts in phase 1
    // (the create POST `.expect(0)` mock verifies no bug was filed).
    assert_eq!(err.exit_code(), 6);
}
