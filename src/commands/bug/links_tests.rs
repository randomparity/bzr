#![expect(clippy::unwrap_used)]

use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::LinksArgs;
use crate::commands::runtime::invocation::CommandContext;
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

/// Mount the graph root on the direct endpoint the root read uses (#719).
/// Related ids are still answered by the batched search endpoint, so a test
/// that mounts a root through `query_param("id", …)` is mounting the wrong one.
async fn mount_root(mock: &wiremock::MockServer, id: u64, node: &Value) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{id}")))
        .respond_with(node_body(&serde_json::json!([node])))
        .mount(mock)
        .await;
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
    mount_root(
        &mock,
        1,
        &serde_json::json!({"id": 1, "summary": "root", "status": "NEW", "depends_on": [2]}),
    )
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
    // 1 -> 2 -> 3 -> 1 (cycle). Root 1 is read directly; 2 and 3 are reached
    // through the batched search read, and 3's edge back to 1 is what the
    // visited set has to absorb.
    mount_root(
        &mock,
        1,
        &serde_json::json!({"id": 1, "summary": "bug1", "status": "NEW", "depends_on": [2]}),
    )
    .await;
    for (id, dep) in [(2u64, 3u64), (3, 1)] {
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
    // ADR 0015's reserved case: the *direct* path answers 200 with an empty
    // result and no error payload, which is the one shape where "no such bug"
    // is what the server actually said.
    Mock::given(method("GET"))
        .and(path("/rest/bug/99"))
        .respond_with(node_body(&serde_json::json!([])))
        .mount(&mock)
        .await;

    let (_out, _err, result) = run(&links_action(99, false, 1, None), OutputFormat::Ndjson).await;
    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::NotFound { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 2);
}

// ── #719: the root read must be able to fault ────────────────────────────
//
// Bugzilla's search endpoint filters a bug the caller cannot see into an empty
// 200 that carries no error at all; the direct endpoint faults with the
// server's own code. Reading the root through search therefore reported a
// permission denial as absence. ADR 0015 reserves `NotFound` for the direct
// path returning an empty result with *no* error payload, which is the case
// `links_root_not_found_is_notfound_exit_2` above still pins.
#[tokio::test]
async fn links_root_permission_denied_is_api_error_not_notfound() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/99"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 102,
            "message": "You are not authorized to access bug #99."
        })))
        .mount(&mock)
        .await;
    // The search endpoint stays mounted and answers the same id with the
    // filtered-out empty list that used to become `bug not found: 99`. The
    // root read must not be satisfied by it.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "99"))
        .respond_with(node_body(&serde_json::json!([])))
        .mount(&mock)
        .await;

    let (_out, _err, result) = run(&links_action(99, false, 1, None), OutputFormat::Ndjson).await;
    let err = result.unwrap_err();
    assert!(
        matches!(err, BzrError::Api { code: 102, .. }),
        "a permission denial must reach the caller as the server's own error: {err:?}"
    );
    assert_eq!(err.exit_code(), 4);
}

// Criterion 3: an omission is fatal only for the root. A related id the search
// endpoint filters out is skipped and the walk still succeeds.
#[tokio::test]
async fn links_related_id_omitted_by_search_is_skipped_not_fatal() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_root(
        &mock,
        1,
        &serde_json::json!({"id": 1, "summary": "root", "status": "NEW", "depends_on": [2, 3]}),
    )
    .await;
    // Both ids are requested in one batch; only 2 comes back. 3 is invisible to
    // this caller, and that omission must not fail the command.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "2"))
        .respond_with(node_body(&serde_json::json!([
            {"id": 2, "summary": "visible dep", "status": "ASSIGNED"}
        ])))
        .mount(&mock)
        .await;

    let (out, _err, result) = run(&links_action(1, false, 1, None), OutputFormat::Ndjson).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "only the visible related bug is emitted: {out}"
    );
    let rec: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(rec["id"], 2);
}

#[tokio::test]
async fn links_discovery_order_independent_of_response_array_order() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
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
    mount_root(
        &mock,
        1,
        &serde_json::json!({
            "id": 1, "summary": "root", "status": "NEW",
            "depends_on": [2], "blocks": [3]
        }),
    )
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
    mount_root(
        &mock,
        5,
        &serde_json::json!({"id": 5, "summary": "lonely", "status": "NEW"}),
    )
    .await;

    let (out, _err, result) = run(&links_action(5, false, 1, None), OutputFormat::Table).await;
    assert!(result.is_ok(), "{:?}", result.err());
    assert!(out.contains("No related bugs for #5."), "{out}");
}
