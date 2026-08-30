#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{handle, validate};
use crate::cli::AdjacencyArgs;
use crate::client::{BugzillaClient, BugzillaClientConfig};
use crate::commands::runtime::invocation::CommandContext;
use crate::test_helpers::CapturedIo;
use crate::types::{ApiMode, AuthMethod, OutputFormat};

fn client(
    server: &MockServer,
    credential: Option<&str>,
    email: Option<&str>,
    retry_max: u32,
) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.uri(),
        credential,
        auth_method: credential.map(|_| AuthMethod::Header),
        api_mode: ApiMode::Rest,
        email_hint: email,
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max,
    })
    .unwrap()
}

fn action(ids: &[&str]) -> AdjacencyArgs {
    AdjacencyArgs {
        ids: ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn complete_bug(id: u64, summary: &str, blocks: &[u64], depends_on: &[u64]) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "summary": summary,
        "status": "NEW",
        "resolution": "",
        "product": "TestProduct",
        "version": "unspecified",
        "assigned_to": "owner@example.invalid",
        "last_change_time": "2026-08-29T00:00:00Z",
        "target_milestone": "---",
        "blocks": blocks,
        "depends_on": depends_on
    })
}

fn success(body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "bugs": [body],
        "faults": []
    }))
}

fn resource_error(code: u16) -> ResponseTemplate {
    ResponseTemplate::new(404).set_body_json(serde_json::json!({
        "error": true,
        "code": code,
        "message": "discarded"
    }))
}

async fn mount_get(server: &MockServer, requested: &str, response: ResponseTemplate, count: u64) {
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .and(query_param("ids", requested))
        .respond_with(response)
        .expect(count)
        .mount(server)
        .await;
}

async fn run(
    client: &BugzillaClient,
    args: &AdjacencyArgs,
) -> (CapturedIo, crate::error::Result<()>) {
    let mut io = CapturedIo::new();
    let result = handle(client, args, OutputFormat::Json, &mut io.writers()).await;
    (io, result)
}

#[test]
fn validation_accepts_i64_max() {
    assert!(validate(&action(&["9223372036854775807"])).is_ok());
}

#[test]
fn validation_accepts_leading_zero_numeric_spelling() {
    assert!(validate(&action(&["0001"])).is_ok());
}

#[test]
fn validation_accepts_duplicate_arguments() {
    assert!(validate(&action(&["1", "1"])).is_ok());
}

#[test]
fn validation_accepts_mixed_numeric_and_alias_requests() {
    assert!(validate(&action(&["1", "release/2026"])).is_ok());
}

#[tokio::test]
async fn success_preserves_duplicates_and_uses_deterministic_first_observation() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        "2",
        success(&complete_bug(2, "numeric wins", &[9, 8, 9], &[4, 3, 4])),
        1,
    )
    .await;
    mount_get(
        &server,
        "alpha",
        success(&complete_bug(2, "alias loses", &[99], &[98])),
        1,
    )
    .await;
    mount_get(
        &server,
        "zeta",
        success(&complete_bug(7, "seven", &[6, 6, 5], &[])),
        1,
    )
    .await;

    let args = action(&["zeta", "0002", "alpha", "2", "alpha"]);
    let (io, result) = run(&client(&server, Some("key"), None, 0), &args).await;
    assert!(result.is_ok(), "{:?}", result.err());
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(
        data["requests"],
        serde_json::json!([
            {"requested": "zeta", "bug_id": 7},
            {"requested": "0002", "bug_id": 2},
            {"requested": "alpha", "bug_id": 2},
            {"requested": "2", "bug_id": 2},
            {"requested": "alpha", "bug_id": 2}
        ])
    );
    assert_eq!(data["bugs"][0]["id"], 2);
    assert_eq!(data["bugs"][0]["summary"], "numeric wins");
    assert_eq!(data["bugs"][0]["blocks"], serde_json::json!([8, 9]));
    assert_eq!(data["bugs"][0]["depends_on"], serde_json::json!([3, 4]));
    assert_eq!(data["bugs"][1]["id"], 7);

    let gets = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/rest/bug/")
        .map(|request| {
            request
                .url
                .query_pairs()
                .find(|(key, _)| key == "ids")
                .unwrap()
                .1
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(gets, ["2", "alpha", "zeta"]);
}

#[tokio::test]
async fn mixed_results_exit_zero() {
    let mixed_server = MockServer::start().await;
    mount_get(
        &mixed_server,
        "1",
        success(&complete_bug(1, "one", &[], &[])),
        1,
    )
    .await;
    mount_get(&mixed_server, "404", resource_error(101), 1).await;
    mount_get(&mixed_server, "missing", resource_error(100), 1).await;
    let (mixed_io, mixed_result) = run(
        &client(&mixed_server, None, None, 0),
        &action(&["missing", "1", "404"]),
    )
    .await;
    assert!(mixed_result.is_ok());
    let mixed = crate::test_helpers::json_envelope_data(mixed_io.out_str());
    assert_eq!(mixed["requests"][0]["error"]["api_code"], 100);
    assert_eq!(mixed["requests"][1]["bug_id"], 1);
    assert_eq!(mixed["requests"][2]["error"]["api_code"], 101);
}

#[tokio::test]
async fn all_failure_results_exit_zero() {
    let failed_server = MockServer::start().await;
    mount_get(&failed_server, "404", resource_error(101), 1).await;
    mount_get(&failed_server, "missing", resource_error(100), 1).await;
    let (failed_io, failed_result) = run(
        &client(&failed_server, None, None, 0),
        &action(&["404", "missing"]),
    )
    .await;
    assert!(failed_result.is_ok());
    let failed = crate::test_helpers::json_envelope_data(failed_io.out_str());
    assert_eq!(failed["bugs"], serde_json::json!([]));
}

#[tokio::test]
async fn fatal_api_code_410_leaves_stdout_empty() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        "1",
        ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "login required"
        })),
        1,
    )
    .await;
    let (io, result) = run(&client(&server, None, None, 0), &action(&["1"])).await;
    assert!(result.is_err());
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn fatal_strict_shape_failure_leaves_stdout_empty() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        "1",
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "blocks": []}],
            "faults": []
        })),
        1,
    )
    .await;
    let (io, result) = run(&client(&server, None, None, 0), &action(&["1"])).await;
    assert!(result.is_err());
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn fatal_transport_failure_leaves_stdout_empty() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        "1",
        success(&complete_bug(1, "too slow", &[], &[]))
            .set_delay(std::time::Duration::from_secs(1)),
        1,
    )
    .await;
    let client = BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.uri(),
        credential: None,
        auth_method: None,
        api_mode: ApiMode::Rest,
        email_hint: None,
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: std::time::Duration::from_millis(10),
        retry_max: 10,
    })
    .unwrap();
    let (io, result) = run(&client, &action(&["1"])).await;
    assert!(result.is_err());
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn credentialed_success_without_email_skips_valid_login() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        "1",
        success(&complete_bug(1, "Bugzilla 5.0", &[], &[])),
        1,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let (io, result) = run(&client(&server, Some("key"), None, 0), &action(&["1"])).await;
    assert!(result.is_ok());
    assert_eq!(
        crate::test_helpers::json_envelope_data(io.out_str())["bugs"][0]["id"],
        1
    );
}

#[tokio::test]
async fn anonymous_inaccessible_is_typed_without_valid_login() {
    let server = MockServer::start().await;
    mount_get(&server, "restricted", resource_error(102), 1).await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let (io, result) = run(&client(&server, None, None, 0), &action(&["restricted"])).await;
    assert!(result.is_ok());
    assert_eq!(
        crate::test_helpers::json_envelope_data(io.out_str())["requests"][0]["error"],
        serde_json::json!({"type": "inaccessible", "api_code": 102})
    );
}

#[tokio::test]
async fn credentialed_inaccessible_without_email_is_fatal() {
    let server = MockServer::start().await;
    mount_get(&server, "restricted", resource_error(102), 1).await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let (io, result) = run(
        &client(&server, Some("key"), None, 0),
        &action(&["restricted"]),
    )
    .await;
    assert!(result.is_err());
    assert!(io.out_str().is_empty());
}

#[tokio::test]
async fn each_credentialed_inaccessible_result_gets_a_fresh_proof() {
    let server = MockServer::start().await;
    mount_get(&server, "first", resource_error(102), 1).await;
    mount_get(&server, "second", resource_error(102), 1).await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
        )
        .with_priority(2)
        .expect(1)
        .mount(&server)
        .await;
    let (io, result) = run(
        &client(&server, Some("key"), Some("user@example.com"), 0),
        &action(&["first", "second"]),
    )
    .await;
    assert!(result.is_err());
    assert!(io.out_str().is_empty());
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path() == "/rest/valid_login")
            .count(),
        2
    );
}

#[tokio::test]
async fn retry_budget_cannot_exceed_one_get_and_one_proof_per_distinct_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(resource_error(102))
        .expect(100)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": 1})))
        .expect(100)
        .mount(&server)
        .await;
    let ids = (1..=100).map(|id| id.to_string()).collect::<Vec<_>>();
    let args = AdjacencyArgs {
        ids: ids
            .iter()
            .rev()
            .cloned()
            .chain(ids.clone())
            .take(100)
            .collect(),
    };
    let (io, result) = run(
        &client(&server, Some("key"), Some("user@example.com"), 10),
        &args,
    )
    .await;
    assert!(result.is_ok());
    assert!(!io.out_str().is_empty());
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 200);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/rest/bug/")
            .count(),
        100
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/rest/valid_login")
            .count(),
        100
    );
}

#[tokio::test]
async fn execute_keeps_adjacency_out_of_field_selection_validation() {
    let server = MockServer::start().await;
    mount_get(&server, "1", success(&complete_bug(1, "one", &[], &[])), 1).await;
    let temp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &temp,
        &format!(
            "default_server = \"test\"\n[servers.test]\nurl = \"{}\"\napi_mode = \"rest\"\n",
            server.uri()
        ),
    );
    let mut io = CapturedIo::new();
    let result = super::super::execute(
        &crate::cli::BugAction::Adjacency(action(&["1"])),
        &CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path)),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "{:?}", result.err());
}
