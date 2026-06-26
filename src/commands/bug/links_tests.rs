#![expect(clippy::unwrap_used)]

use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::LinksArgs;
use crate::commands::runtime::context::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{setup_test_env, CapturedIo};
use crate::types::bug::LinkRelation;
use crate::types::OutputFormat;

fn links_action(
    id: u64,
    recursive: bool,
    depth: u32,
    relation: Option<LinkRelation>,
) -> crate::cli::BugAction {
    crate::cli::BugAction::Links(LinksArgs {
        id,
        recursive,
        depth,
        relation,
    })
}

fn node_body(values: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({ "bugs": values }))
}

async fn run(
    action: &crate::cli::BugAction,
    format: OutputFormat,
) -> (String, String, crate::error::Result<()>) {
    let mut io = CapturedIo::new();
    let result = crate::commands::bug::execute(
        action,
        &CommandContext::new(None, format, None),
        &mut io.writers(),
    )
    .await;
    (io.out_str().to_string(), io.err_str().to_string(), result)
}

#[tokio::test]
async fn links_one_hop_emits_single_depth1_record() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "1"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 1, "summary": "root", "status": "NEW", "depends_on": [2]}
        ])))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "2"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 2, "summary": "dep", "status": "ASSIGNED"}
        ])))
        .mount(&mock)
        .await;

    let (out, _err, result) = run(&links_action(1, false, 1, None), OutputFormat::Ndjson).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1);
    let rec: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(rec["id"], 2);
    assert_eq!(rec["relation"], "depends_on");
    assert_eq!(rec["direction"], "out");
    assert_eq!(rec["depth"], 1);
}

#[tokio::test]
async fn links_recursive_cycle_visits_each_once_with_depth() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // 1 -> 2 -> 3 -> 1 (cycle)
    for (id, dep) in [(1u64, 2u64), (2, 3), (3, 1)] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("id", id.to_string()))
            .respond_with(node_body(&serde_json::json!([
                {"id": id, "summary": format!("bug{id}"), "status": "NEW", "depends_on": [dep]}
            ])))
            .mount(&mock)
            .await;
    }

    let (out, _err, result) = run(&links_action(1, true, 5, None), OutputFormat::Ndjson).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let recs: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        recs.len(),
        2,
        "only 2 and 3, root 1 never re-emitted: {out}"
    );
    assert_eq!(recs[0]["id"], 2);
    assert_eq!(recs[0]["depth"], 1);
    assert_eq!(recs[1]["id"], 3);
    assert_eq!(recs[1]["depth"], 2);
}

#[tokio::test]
async fn links_root_not_found_is_notfound_exit_2() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "99"))
        .respond_with(node_body(&serde_json::json!([])))
        .mount(&mock)
        .await;

    let (_out, _err, result) = run(&links_action(99, false, 1, None), OutputFormat::Ndjson).await;
    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::NotFound { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 2);
}

#[tokio::test]
async fn links_discovery_order_independent_of_response_array_order() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "1"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 1, "summary": "root", "status": "NEW", "depends_on": [3, 2]}
        ])))
        .mount(&mock)
        .await;
    // The level-1 batch requests ids [2,3]; server returns them reversed.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "2"))
        .and(query_param("id", "3"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 3, "summary": "three", "status": "NEW"},
            {"id": 2, "summary": "two", "status": "NEW"}
        ])))
        .mount(&mock)
        .await;

    let (out, _err, result) = run(&links_action(1, false, 1, None), OutputFormat::Ndjson).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let recs: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(recs.len(), 2);
    assert_eq!(
        recs[0]["id"], 2,
        "ascending id order regardless of response: {out}"
    );
    assert_eq!(recs[1]["id"], 3);
}

#[tokio::test]
async fn links_relation_filter_restricts_output() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "1"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 1, "summary": "root", "status": "NEW", "depends_on": [2], "blocks": [3]}
        ])))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "2"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 2, "summary": "dep", "status": "NEW"}
        ])))
        .mount(&mock)
        .await;

    let action = links_action(1, false, 1, Some(LinkRelation::DependsOn));
    let (out, _err, result) = run(&action, OutputFormat::Ndjson).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let recs: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0]["id"], 2);
    assert_eq!(recs[0]["relation"], "depends_on");
}

#[tokio::test]
async fn links_no_relations_table_prints_message() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "5"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 5, "summary": "lonely", "status": "NEW"}
        ])))
        .mount(&mock)
        .await;

    let (out, _err, result) = run(&links_action(5, false, 1, None), OutputFormat::Table).await;
    assert!(result.is_ok(), "{:?}", result.err());
    assert!(out.contains("No related bugs for #5."), "{out}");
}
