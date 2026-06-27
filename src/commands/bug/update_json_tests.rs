#![expect(clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::commands::runtime::input::from_json::JsonOneOrMany;
use crate::commands::runtime::invocation::CommandContext;
use crate::types::{OutputFormat, ProgressFormat};

use super::super::update::BugUpdateDraft;
use super::JsonUpdateRequest;

fn sample_update_request(id: u64) -> JsonUpdateRequest {
    JsonUpdateRequest {
        id,
        expect_unchanged_since: None,
        params: crate::types::bug::UpdateBugParams::default(),
    }
}

#[tokio::test]
async fn batch_update_emits_batch_then_done() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    for id in [1u64, 2] {
        Mock::given(method("PUT"))
            .and(path(format!("/rest/bug/{id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"bugs": [{"id": id, "changes": {}}]})),
            )
            .mount(&mock)
            .await;
    }
    let requests = vec![sample_update_request(1), sample_update_request(2)];
    let ctx = CommandContext::new(None, OutputFormat::Json, None)
        .with_progress(Some(ProgressFormat::Ndjson));
    let mut io = crate::test_helpers::CapturedIo::new();
    super::update_many_from_json(&requests, &ctx, &mut io.writers())
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
async fn batch_update_partial_failure_emits_no_done() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 1, "changes": {}}]})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let requests = vec![sample_update_request(1), sample_update_request(2)];
    let ctx = CommandContext::new(None, OutputFormat::Json, None)
        .with_progress(Some(ProgressFormat::Ndjson));
    let mut io = crate::test_helpers::CapturedIo::new();
    let res = super::update_many_from_json(&requests, &ctx, &mut io.writers()).await;
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
fn bug_update_input_schema_matches_parser_keys() {
    let schema_keys: BTreeSet<String> =
        schema_object_properties("bug-update-input", "bugUpdateInputObject")
            .keys()
            .cloned()
            .collect();
    let parser_keys = parser_fields_from_unknown_error::<BugUpdateDraft>();

    assert_eq!(schema_keys, parser_keys);
}

#[test]
fn bug_update_input_schema_examples_parse() {
    for (key, property_schema) in
        schema_object_properties("bug-update-input", "bugUpdateInputObject")
    {
        let example = property_schema
            .get("examples")
            .and_then(serde_json::Value::as_array)
            .and_then(|examples| examples.first())
            .unwrap_or_else(|| panic!("bug-update-input property '{key}' missing example"))
            .clone();
        let mut object = serde_json::Map::new();
        object.insert(key.clone(), example);

        serde_json::from_value::<BugUpdateDraft>(serde_json::Value::Object(object))
            .unwrap_or_else(|err| panic!("schema example for '{key}' did not parse: {err}"));
    }
}

#[test]
fn bug_update_input_schema_array_items_require_id() {
    let schema = schema_value("bug-update-input");
    let required = schema
        .pointer("/$defs/bugUpdateInputArrayItem/allOf/1/required")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("bug-update-input array item schema missing required list"));

    assert!(required.iter().any(|field| field.as_str() == Some("id")));
}

#[test]
fn parse_json_updates_object_and_array_shapes() {
    assert!(matches!(
        crate::commands::runtime::input::from_json::parse_one_or_many::<BugUpdateDraft>(
            r#"{"id":1,"status":"ASSIGNED"}"#
        )
        .unwrap(),
        JsonOneOrMany::One(_)
    ));

    match crate::commands::runtime::input::from_json::parse_one_or_many::<BugUpdateDraft>(
        r#"[{"id":1,"status":"ASSIGNED"},{"id":2,"priority":"high"}]"#,
    )
    .unwrap()
    {
        JsonOneOrMany::Many(v) => assert_eq!(v.len(), 2),
        JsonOneOrMany::One(_) => panic!("array should parse as Many"),
    }

    assert!(matches!(
        crate::commands::runtime::input::from_json::parse_one_or_many::<BugUpdateDraft>(
            r#"[{"id":1,"status":"ASSIGNED"}]"#
        )
        .unwrap(),
        JsonOneOrMany::Many(_)
    ));
}

// ── cli_comment_uses_stdin (→ false and || → &&) ─────────────────────────────
//
// Mutant line 39: replace return with `false` — both tests would then pass a
// false value through, but the assertion checks for `true`, so they fail.
// Mutant line 40: replace `||` with `&&` — comment-only or comment_file-only
// cases would then return false; the tests below each use only ONE source.

#[test]
fn cli_comment_uses_stdin_returns_true_for_comment_dash() {
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("-".into()),
        ..Default::default()
    };
    assert!(
        super::cli_comment_uses_stdin(&args),
        "comment = \"-\" must make cli_comment_uses_stdin return true"
    );
}

#[test]
fn cli_comment_uses_stdin_returns_true_for_comment_file_dash() {
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment_file: Some(std::path::PathBuf::from("-")),
        ..Default::default()
    };
    assert!(
        super::cli_comment_uses_stdin(&args),
        "comment_file = \"-\" must make cli_comment_uses_stdin return true"
    );
}

#[test]
fn cli_comment_uses_stdin_returns_false_when_no_stdin_source() {
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("inline body".into()),
        ..Default::default()
    };
    assert!(
        !super::cli_comment_uses_stdin(&args),
        "non-stdin comment must not trigger stdin detection"
    );
}

// ── reject_cli_stdin_comment_source (→ Ok(())) ──────────────────────────────
//
// Mutant line 48: replace function body with Ok(()) — the first test must fail
// because it expects Err.

#[test]
fn reject_cli_stdin_comment_source_rejects_stdin_json_with_stdin_comment() {
    // from_json arg is "-" (stdin) and comment also uses stdin:
    // the function must return Err regardless of is_array.
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("-".into()),
        ..Default::default()
    };
    let result = super::reject_cli_stdin_comment_source(&args, super::JsonUpdateInputSource::Stdin);
    assert!(
        result.is_err(),
        "stdin JSON + stdin comment must be rejected: got {result:?}"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("--from-json -"),
        "error must mention --from-json -, got: {msg}"
    );
}

#[test]
fn reject_cli_stdin_comment_source_rejects_array_json_with_stdin_comment() {
    // from_json arg is a file path but is_array=true and comment uses stdin:
    // arrays cannot consume stdin for a single comment body.
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("-".into()),
        ..Default::default()
    };
    let result =
        super::reject_cli_stdin_comment_source(&args, super::JsonUpdateInputSource::FileArray);
    assert!(
        result.is_err(),
        "array JSON + stdin comment must be rejected: got {result:?}"
    );
}

#[test]
fn reject_cli_stdin_comment_source_allows_file_json_with_stdin_comment_single() {
    // from_json arg is a regular file and is_array=false: single-object JSON
    // can safely combine with stdin comment (the file is read first).
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("-".into()),
        ..Default::default()
    };
    let result =
        super::reject_cli_stdin_comment_source(&args, super::JsonUpdateInputSource::FileObject);
    assert!(
        result.is_ok(),
        "file JSON + stdin comment (single) must be allowed: got {result:?}"
    );
}

#[test]
fn reject_cli_stdin_comment_source_allows_no_stdin_comment() {
    // Neither comment source uses stdin: any from_json arg is fine.
    let args = crate::cli::UpdateArgs {
        ids: vec![1],
        comment: Some("normal body".into()),
        ..Default::default()
    };
    assert!(
        super::reject_cli_stdin_comment_source(&args, super::JsonUpdateInputSource::Stdin).is_ok()
    );
    assert!(
        super::reject_cli_stdin_comment_source(&args, super::JsonUpdateInputSource::FileArray)
            .is_ok()
    );
}
