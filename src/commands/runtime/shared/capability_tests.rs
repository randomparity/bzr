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
    assert_eq!(
        err.structured_detail()
            .get("capability_status")
            .and_then(|v| v.as_str()),
        Some("absent")
    );
    let message = err.to_string();
    assert!(message.contains("saved search 'triage'"), "{message}");
    assert!(message.contains(RED_HAT_EXTENSION), "{message}");
    assert!(
        message.contains("'test'"),
        "must name the server: {message}"
    );
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
    assert_eq!(
        err.structured_detail()
            .get("capability_status")
            .and_then(|v| v.as_str()),
        Some("undetermined"),
        "a failed probe must be machine-distinguishable from an absent extension"
    );
    let message = err.to_string();
    assert!(message.contains("could not determine"), "{message}");
    assert!(!message.contains("does not implement"), "{message}");
    assert!(
        message.contains("saved search"),
        "the probe-failure path must still name the operation: {message}"
    );
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

/// Only capabilities bzr acts on are cached. The probe response is
/// server-controlled and unbounded; persisting it verbatim would write
/// arbitrary server text into the user's config for no gain.
#[tokio::test]
async fn only_known_capabilities_are_persisted() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let noise: Vec<String> = (0..50).map(|i| format!("Noise{i}")).collect();
    let mut names: Vec<&str> = noise.iter().map(String::as_str).collect();
    names.push(RED_HAT_EXTENSION);
    mount_extensions(&mock, &names, 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .unwrap();

    let config = crate::config::Config::load_at(ctx.config_path_override()).unwrap();
    let (_, srv) = config.resolve_server(None).unwrap();
    assert_eq!(
        srv.server_extensions.as_deref(),
        Some([RED_HAT_EXTENSION.to_string()].as_slice()),
        "server-advertised names outside the known set must not be persisted"
    );
}

/// An inline `--server-url` connection has no config entry: it must neither
/// trust nor write the named server's cached answer.
#[tokio::test]
async fn inline_server_neither_reads_nor_writes_the_cache() {
    let (_lock, mock, tmp) = setup_test_env().await;
    // Seed the *named* server's cache as supporting the capability.
    let config_path = crate::config::Config::path_at(None).unwrap();
    crate::config::Config::update_locked_at(Some(&config_path), |config| {
        if let Some(srv) = config.servers.get_mut("test") {
            srv.server_extensions = Some(vec![RED_HAT_EXTENSION.to_string()]);
        }
        Ok(())
    })
    .unwrap();
    let _ = &tmp;

    // The inline server advertises nothing.
    mount_extensions(&mock, &[], 1).await;
    let ctx = ctx().with_inline_server(Some(crate::commands::runtime::invocation::InlineServer {
        url: mock.uri(),
        api_key_env: None,
        email: None,
        tls: crate::commands::runtime::invocation::InlineTlsOptions::default(),
    }));
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("the inline server advertises nothing, so it must be refused");
    assert_eq!(err.exit_code(), 15);

    let after = crate::config::Config::load_at(Some(&config_path)).unwrap();
    let (_, srv) = after.resolve_server(None).unwrap();
    assert_eq!(
        srv.server_extensions.as_deref(),
        Some([RED_HAT_EXTENSION.to_string()].as_slice()),
        "an inline invocation must not overwrite the named server's cache"
    );
}
