#![expect(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::test_client;
use crate::commands::runtime::invocation::CommandContext;
use crate::test_helpers::write_config_to;
use crate::types::output::OutputFormat;

use super::{validate_bug_fields, BugzillaClient};

fn keys(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).to_string()).collect()
}

/// Config naming the same server as [`test_client`], so the cache lookup and
/// the persist target line up with the client's resolved server name.
fn write_config(tmp: &tempfile::TempDir, url: &str, extra: &str) -> PathBuf {
    write_config_to(
        tmp,
        &format!(
            "default_server = \"test\"\n\n[servers.test]\nurl = \"{url}\"\napi_key = \"test-key\"\n{extra}\n"
        ),
    )
}

fn ctx_for(config_path: &Path) -> CommandContext {
    CommandContext::new(Some("test"), OutputFormat::Json, None)
        .with_config_path_override(Some(config_path.to_path_buf()))
}

fn cached_names(config_path: &Path) -> Option<Vec<String>> {
    crate::config::Config::load_at(Some(config_path))
        .unwrap()
        .servers
        .get("test")
        .unwrap()
        .bug_field_names
        .clone()
}

async fn mount_catalogue(server: &MockServer, names: &[&str], expected_calls: u64) {
    let fields: Vec<serde_json::Value> = names
        .iter()
        .map(|n| serde_json::json!({"name": n}))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": fields
        })))
        .expect(expected_calls)
        .mount(server)
        .await;
}

async fn setup(
    names: &[&str],
    expected_calls: u64,
) -> (MockServer, tempfile::TempDir, BugzillaClient) {
    let server = MockServer::start().await;
    mount_catalogue(&server, names, expected_calls).await;
    let tmp = tempfile::TempDir::new().unwrap();
    let client = test_client(&server.uri());
    (server, tmp, client)
}

#[tokio::test]
async fn empty_key_set_never_probes() {
    let (server, tmp, client) = setup(&["whiteboard"], 0).await;
    let config_path = write_config(&tmp, &server.uri(), "");
    validate_bug_fields(&client, &ctx_for(&config_path), &BTreeSet::new())
        .await
        .expect("no keys means no work");
}

#[tokio::test]
async fn declared_key_is_accepted_and_the_names_are_persisted() {
    let (server, tmp, client) = setup(&["whiteboard", "cf_release"], 1).await;
    let config_path = write_config(&tmp, &server.uri(), "");

    validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_release"]))
        .await
        .expect("declared key is accepted");

    assert_eq!(
        cached_names(&config_path),
        Some(vec!["cf_release".to_string(), "whiteboard".to_string()]),
        "a successful probe caches the sorted names"
    );
}

#[tokio::test]
async fn undeclared_key_is_refused_at_exit_seven() {
    let (server, tmp, client) = setup(&["whiteboard"], 1).await;
    let config_path = write_config(&tmp, &server.uri(), "");

    let err = validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_relase"]))
        .await
        .unwrap_err();

    assert_eq!(err.exit_code(), 7);
    let message = err.to_string();
    assert!(message.contains("cf_relase"), "names the field: {message}");
    assert!(
        message.contains("bzr server capabilities"),
        "points at discovery: {message}"
    );
}

/// A key already in the cached list is answered without a request. The mock
/// asserts zero calls, so a regression that always probes fails here.
#[tokio::test]
async fn cached_names_answer_without_a_probe() {
    let (server, tmp, client) = setup(&["whiteboard"], 0).await;
    let config_path = write_config(
        &tmp,
        &server.uri(),
        "bug_field_names = [\"cf_release\", \"whiteboard\"]",
    );

    validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_release"]))
        .await
        .expect("cache hit is accepted");
}

/// The cache is a fast path, never an authority: a key it does not list forces
/// a fresh probe, so a field added on the server since the list was written is
/// accepted rather than wrongly rejected.
#[tokio::test]
async fn cache_miss_reprobes_and_accepts_a_newly_declared_field() {
    let (server, tmp, client) = setup(&["whiteboard", "cf_new"], 1).await;
    let config_path = write_config(&tmp, &server.uri(), "bug_field_names = [\"whiteboard\"]");

    validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_new"]))
        .await
        .expect("a stale cache must not reject a declared field");

    assert_eq!(
        cached_names(&config_path),
        Some(vec!["cf_new".to_string(), "whiteboard".to_string()]),
        "the refreshed list replaces the stale one"
    );
}

/// A probe failure is not an absent field. The write is refused, the message
/// names the catalogue probe, and it never claims the server lacks the field.
#[tokio::test]
async fn probe_failure_refuses_the_write_with_its_own_diagnostic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
        .mount(&server)
        .await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");
    let client = test_client(&server.uri());

    let err = validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_release"]))
        .await
        .unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("bug field catalogue was not retrieved"),
        "names the probe: {message}"
    );
    assert!(
        message.contains("no changes were sent"),
        "states the write was refused: {message}"
    );
    assert!(
        !message.contains("does not declare"),
        "must not read as an absent field: {message}"
    );
    assert_ne!(
        err.exit_code(),
        7,
        "a probe failure is distinguishable from an undeclared field"
    );
    assert_eq!(
        cached_names(&config_path),
        None,
        "a failed probe caches nothing"
    );
}

/// An inline `--server-url` invocation has no config entry. Validation must
/// still run and still refuse an undeclared key; persistence is the no-op.
#[tokio::test]
async fn server_absent_from_config_validates_without_persisting() {
    let (server, tmp, client) = setup(&["whiteboard"], 1).await;
    let config_path = write_config_to(
        &tmp,
        &format!(
            "default_server = \"other\"\n\n[servers.other]\nurl = \"{}\"\napi_key = \"k\"\n",
            server.uri()
        ),
    );

    let err = validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_relase"]))
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 7);

    let config = crate::config::Config::load_at(Some(&config_path)).unwrap();
    assert!(
        !config.servers.contains_key("test"),
        "an absent server is not resurrected by the persist"
    );
}

/// Bugzilla's catalogue calls the whiteboard `status_whiteboard`, but
/// `Bug.update` takes `whiteboard`. A REST name bzr already models is accepted
/// without a probe, so the alias gap never reaches the user — this is the case
/// the python-bugzilla comparison drives.
#[tokio::test]
async fn a_rest_name_bzr_models_is_accepted_without_a_probe() {
    let (server, tmp, client) = setup(&["status_whiteboard"], 0).await;
    let config_path = write_config(&tmp, &server.uri(), "");

    validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["whiteboard"]))
        .await
        .expect("a REST name bzr models needs no catalogue round trip");
}

/// A mixed key set still probes once, for the keys bzr does not model.
#[tokio::test]
async fn a_mixed_key_set_probes_only_for_the_unknown_keys() {
    let (server, tmp, client) = setup(&["status_whiteboard", "cf_release"], 1).await;
    let config_path = write_config(&tmp, &server.uri(), "");

    validate_bug_fields(
        &client,
        &ctx_for(&config_path),
        &keys(&["whiteboard", "cf_release"]),
    )
    .await
    .expect("both keys are acceptable");
}

/// The cache has no role in the answer, so a config it cannot write must not
/// turn an otherwise valid write into an error.
#[tokio::test]
async fn an_unwritable_config_does_not_fail_the_validation() {
    let (server, tmp, client) = setup(&["cf_release"], 1).await;
    let config_path = write_config(&tmp, &server.uri(), "");
    let dir = config_path.parent().unwrap().to_path_buf();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    }

    let result = validate_bug_fields(&client, &ctx_for(&config_path), &keys(&["cf_release"])).await;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    assert!(
        result.is_ok(),
        "a failed cache write must not fail the validation: {result:?}"
    );
    assert_eq!(
        cached_names(&config_path),
        None,
        "the write really did fail, so the assertion above is not vacuous"
    );
}
