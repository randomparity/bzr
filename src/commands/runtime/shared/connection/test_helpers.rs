#![expect(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::test_helpers::write_config_to;

use super::target::ConnectContext;

/// The `[servers.test]` lines that put a connect on the fully-cached path: a
/// cached auth method carrying a trusted provenance stamp, plus a cached API
/// mode. The stamp is load-bearing — without it the entry is treated as written
/// before the differential probe and re-detects instead (ADR-0066), which
/// changes the request counts a cached-path test asserts on.
pub(super) const CACHED_SERVER: &str = concat!(
    "auth_method = \"header\"\n",
    "auth_method_source = \"differential-probe\"\n",
    "api_mode = \"rest\"",
);

pub(super) fn load_config(path: &Path) -> crate::config::Config {
    crate::config::Config::load_at(Some(path)).unwrap()
}

pub(super) fn connect_context(
    server_name: &str,
    url: &str,
    api_override: Option<crate::types::ApiMode>,
    config_path: Option<PathBuf>,
) -> ConnectContext {
    ConnectContext {
        server_name: server_name.to_string(),
        url: url.to_string(),
        api_key: Some("test-key".to_string()),
        token: None,
        email: None,
        api_override,
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
        config_path_override: config_path,
        persist: true,
    }
}

/// Write a config TOML with the given extra fields injected into the
/// `[servers.test]` table to an isolated temp path and return that path. No env
/// mutation: pass the path via `CommandContext::with_config_path_override` so the
/// test needs no `ENV_LOCK`.
pub(super) fn write_config(tmp: &tempfile::TempDir, server_url: &str, extra: &str) -> PathBuf {
    let config_content = format!(
        r#"
default_server = "test"

[servers.test]
url = "{server_url}"
api_key = "test-key"
{extra}
"#,
    );
    write_config_to(tmp, &config_content)
}

/// Mount the standard whoami + version mocks used by auth/version detection.
pub(super) async fn mount_detection_mocks(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(server)
        .await;
}
