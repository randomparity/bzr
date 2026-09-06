#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{require_server_capability, RED_HAT_EXTENSION};
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::shared::connect_and_configure;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn ctx() -> CommandContext {
    CommandContext::new(None, OutputFormat::Json, None)
}

/// `{"extensions": {...}}` for the given names.
fn extensions_body(names: &[&str]) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = names
        .iter()
        .map(|n| ((*n).to_string(), serde_json::json!({ "version": "0.1" })))
        .collect();
    serde_json::json!({ "extensions": map })
}

async fn mount_extensions(mock: &wiremock::MockServer, names: &[&str], expect: u64) {
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(extensions_body(names)))
        .expect(expect)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn advertised_capability_is_allowed() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_extensions(&mock, &[RED_HAT_EXTENSION, "Voting"], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let result = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search").await;

    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn absent_capability_is_refused_with_exit_15() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_extensions(&mock, &["Voting"], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search 'triage'")
        .await
        .expect_err("an unadvertised capability must be refused");

    assert_eq!(err.exit_code(), 15);
    assert_eq!(err.error_type(), "unsupported_server_capability");
    let message = err.to_string();
    assert!(message.contains("saved search 'triage'"), "{message}");
    assert!(message.contains(RED_HAT_EXTENSION), "{message}");
}

/// An empty extension map is a real answer, not a failed probe: the server
/// said it has no extensions.
#[tokio::test]
async fn empty_extension_map_is_refused_not_undetermined() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_extensions(&mock, &[], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("an empty extension map does not advertise the capability");

    assert!(err.to_string().contains("does not implement"), "{err}");
}

/// A failed probe must not be reported as an absent extension — that would
/// turn a transient network fault into a claim about the server.
#[tokio::test]
async fn probe_failure_is_undetermined_not_absent() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("a failed probe must not be treated as support");

    assert_eq!(err.exit_code(), 15);
    let message = err.to_string();
    assert!(message.contains("could not determine"), "{message}");
    assert!(!message.contains("does not implement"), "{message}");
}

/// The second call reads the cached list, so the probe runs once per server.
/// `.expect(1)` on the mount is what proves it.
#[tokio::test]
async fn probed_extensions_are_cached_for_the_next_call() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_extensions(&mock, &[RED_HAT_EXTENSION], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .unwrap();
    require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .unwrap();

    let config = crate::config::Config::load_at(ctx.config_path_override()).unwrap();
    let (_, srv) = config.resolve_server(None).unwrap();
    assert_eq!(
        srv.server_extensions.as_deref(),
        Some([RED_HAT_EXTENSION.to_string()].as_slice())
    );
}
