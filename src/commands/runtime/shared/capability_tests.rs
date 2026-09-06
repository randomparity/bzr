#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{require_server_capability, RED_HAT_EXTENSION};
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::shared::connect_and_configure;
use crate::error::{CAPABILITY_ABSENT, CAPABILITY_UNDETERMINED};
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
            srv.server_extensions_url = Some(srv.url.clone());
            srv.server_extensions_known = Some(vec![RED_HAT_EXTENSION.to_string()]);
        }
        Ok(())
    })
    .unwrap();
    let _ = &tmp;

    // The inline server has no config entry, so connect_and_configure detects
    // its API mode rather than reading a cached one. Since the probe now
    // follows the resolved transport (ADR-0052, amended 2026-09-06), an
    // undetectable version would fall back to XmlRpc and this REST mock would
    // never fire — so pin a REST-capable version. The cache assertions below
    // are what this test is actually about.
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "version": "5.2"
        })))
        .mount(&mock)
        .await;
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

/// A cached answer is bound to the URL it was probed from. Re-pointing a
/// server name at another host must re-probe, or the gate fails open on
/// capabilities the new host never advertised.
#[tokio::test]
async fn cache_probed_from_a_different_url_is_not_trusted() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let config_path = crate::config::Config::path_at(None).unwrap();
    crate::config::Config::update_locked_at(Some(&config_path), |config| {
        if let Some(srv) = config.servers.get_mut("test") {
            srv.server_extensions = Some(vec![RED_HAT_EXTENSION.to_string()]);
            srv.server_extensions_url = Some("https://elsewhere.example".to_string());
            srv.server_extensions_known = Some(vec![RED_HAT_EXTENSION.to_string()]);
        }
        Ok(())
    })
    .unwrap();

    // The server actually pointed at advertises nothing.
    mount_extensions(&mock, &[], 1).await;
    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("a cache from another URL must not satisfy the gate");
    assert_eq!(err.exit_code(), 15);

    let after = crate::config::Config::load_at(Some(&config_path)).unwrap();
    let (_, srv) = after.resolve_server(None).unwrap();
    assert_eq!(srv.server_extensions.as_deref(), Some([].as_slice()));
    assert_eq!(srv.server_extensions_url.as_deref(), Some(&*mock.uri()));
    assert_eq!(
        srv.server_extensions_known.as_deref(),
        Some([RED_HAT_EXTENSION.to_string()].as_slice())
    );
}

/// A cached empty list is a hit, not a miss: the server answered and
/// advertised nothing. `.expect(0)` proves no second probe is issued.
#[tokio::test]
async fn cached_empty_list_is_a_hit_and_refuses_without_probing() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let config_path = crate::config::Config::path_at(None).unwrap();
    crate::config::Config::update_locked_at(Some(&config_path), |config| {
        if let Some(srv) = config.servers.get_mut("test") {
            srv.server_extensions = Some(vec![]);
            srv.server_extensions_url = Some(srv.url.clone());
            srv.server_extensions_known = Some(vec![RED_HAT_EXTENSION.to_string()]);
        }
        Ok(())
    })
    .unwrap();
    mount_extensions(&mock, &[RED_HAT_EXTENSION], 0).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("a cached empty list means the capability is absent");
    assert_eq!(err.exit_code(), 15);
}

/// A cache written against a different capability allowlist cannot answer for
/// a capability added later, so it must be treated as a miss.
#[tokio::test]
async fn cache_written_against_a_different_allowlist_is_not_trusted() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let config_path = crate::config::Config::path_at(None).unwrap();
    crate::config::Config::update_locked_at(Some(&config_path), |config| {
        if let Some(srv) = config.servers.get_mut("test") {
            srv.server_extensions = Some(vec![]);
            srv.server_extensions_url = Some(srv.url.clone());
            srv.server_extensions_known = Some(vec!["SomethingElse".to_string()]);
        }
        Ok(())
    })
    .unwrap();
    mount_extensions(&mock, &[RED_HAT_EXTENSION], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect("a stale-allowlist cache must be re-probed, not trusted");
}

// ── Transport-aware probe (ADR-0052, amended 2026-09-06) ───────────────

const XMLRPC_ADVERTISED: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct>",
    r"<member><name>RedHat</name><value><struct>",
    r"<member><name>version</name><value><string>1.0</string></value></member>",
    r"</struct></value></member>",
    r"</struct></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const XMLRPC_EMPTY: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct /></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

fn ctx_with_api(api: crate::types::ApiMode) -> CommandContext {
    CommandContext::new(None, OutputFormat::Json, Some(api))
}

async fn mount_xmlrpc(mock: &wiremock::MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(response)
        .mount(mock)
        .await;
}

/// Mount `/rest/extensions` returning the Bugzilla error envelope a real
/// server sends for an absent endpoint, at the given status.
async fn mount_rest_failure(mock: &wiremock::MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(status).set_body_json(serde_json::json!({
                "error": true,
                "code": 32614,
                "message": "A REST API resource was not found for 'GET /extensions'."
            })),
        )
        .mount(mock)
        .await;
}

/// The issue itself: a server whose REST surface cannot answer, whose XML-RPC
/// surface advertises the extension. Before this change the capability was
/// permanently undetermined here.
#[tokio::test]
async fn capability_established_over_xmlrpc_when_rest_is_unreachable() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_rest_failure(&mock, 503).await;
    mount_xmlrpc(
        &mock,
        ResponseTemplate::new(200).set_body_string(XMLRPC_ADVERTISED),
    )
    .await;

    let ctx = ctx_with_api(crate::types::ApiMode::XmlRpc);
    let client = connect_and_configure(&ctx).await.unwrap();

    require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect("XML-RPC must establish the capability when REST cannot be reached");
}

#[tokio::test]
async fn capability_absent_over_xmlrpc() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_xmlrpc(
        &mock,
        ResponseTemplate::new(200).set_body_string(XMLRPC_EMPTY),
    )
    .await;

    let ctx = ctx_with_api(crate::types::ApiMode::XmlRpc);
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("an empty extension list must be refused");

    // `absent`, not `undetermined`: this is what proves the XML-RPC response
    // was received and parsed rather than the probe merely failing.
    assert_eq!(
        err.structured_detail()
            .get("capability_status")
            .and_then(|v| v.as_str()),
        Some(CAPABILITY_ABSENT)
    );
}

#[tokio::test]
async fn capability_absent_message_does_not_name_rest_path() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_extensions(&mock, &["Voting"], 1).await;

    let ctx = ctx();
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("an unadvertised capability must be refused");

    let message = err.to_string();
    assert!(!message.contains("/rest/extensions"), "{message}");
}

#[tokio::test]
async fn capability_undetermined_message_does_not_name_rest() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_xmlrpc(&mock, ResponseTemplate::new(500)).await;

    let ctx = ctx_with_api(crate::types::ApiMode::XmlRpc);
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("a failed probe must be refused as undetermined");

    let message = err.to_string();
    assert_eq!(
        err.structured_detail()
            .get("capability_status")
            .and_then(|v| v.as_str()),
        Some(CAPABILITY_UNDETERMINED)
    );
    assert!(!message.contains("/rest/extensions"), "{message}");
    assert!(!message.contains("REST surface"), "{message}");
}

/// In Hybrid both transports are attempted, so the refusal must say so. The
/// warn-level log of the REST failure is a trace event, not the error body, and a
/// user on a REST-first connection reading only an XML-RPC error would
/// reasonably conclude bzr never tried REST.
#[tokio::test]
async fn capability_undetermined_in_hybrid_names_both_attempts() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_rest_failure(&mock, 404).await;
    mount_xmlrpc(&mock, ResponseTemplate::new(500)).await;

    let ctx = ctx_with_api(crate::types::ApiMode::Hybrid);
    let client = connect_and_configure(&ctx).await.unwrap();
    let err = require_server_capability(&ctx, &client, RED_HAT_EXTENSION, "saved search")
        .await
        .expect_err("both transports failing must be refused");

    let message = err.to_string();
    assert!(message.contains("REST"), "{message}");
    assert!(message.contains("XML-RPC"), "{message}");
}
