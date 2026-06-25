//! Shared test utilities used by both unit tests (`src/`) and integration tests (`tests/`).
//!
//! Tests that set `XDG_CONFIG_HOME` must hold `ENV_LOCK` to avoid races.

use std::collections::{BTreeMap, BTreeSet};

/// Acquire `ENV_LOCK`, start a mock server, create a temp dir, and configure it.
/// Returns the guard, mock server, and temp dir (all must stay alive for the test).
///
/// # Panics
///
/// Panics if the temp directory cannot be created.
#[expect(clippy::unwrap_used)]
pub async fn setup_test_env() -> (
    tokio::sync::MutexGuard<'static, ()>,
    wiremock::MockServer,
    tempfile::TempDir,
) {
    let lock = super::ENV_LOCK.lock().await;
    let mock = wiremock::MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());
    (lock, mock, tmp)
}

/// The TOML body shared by `setup_config` and `setup_isolated_env`: one
/// `[servers.test]` server with a literal api key and cached auth/API mode, so
/// connect takes the cached path without re-detecting.
fn default_test_config(server_url: &str) -> String {
    format!(
        r#"
default_server = "test"

[servers.test]
url = "{server_url}"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
"#,
    )
}

/// Write `contents` to `<tmp>/bzr/config.toml`, hardening permissions on unix,
/// and return the config file path. Unlike `setup_config`, this performs **no**
/// process-environment mutation: pass the returned path to
/// `CommandContext::with_config_path_override` (or the `--config` flag) so config
/// resolution never consults `XDG_CONFIG_HOME`, and the test needs no `ENV_LOCK`.
///
/// # Panics
///
/// Panics if the config directory or file cannot be created.
#[expect(clippy::unwrap_used)]
pub fn write_config_to(tmp: &tempfile::TempDir, contents: &str) -> std::path::PathBuf {
    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    std::fs::write(&config_path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    config_path
}

/// Write a test config file to the given temp directory and point
/// `XDG_CONFIG_HOME` at it. Caller must hold `ENV_LOCK`.
///
/// # Panics
///
/// Panics if the config directory or file cannot be created.
pub fn setup_config(tmp: &tempfile::TempDir, server_url: &str) {
    write_config_to(tmp, &default_test_config(server_url));
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
}

/// Lock-free analogue of `setup_test_env`: start a mock server, write the default
/// `[servers.test]` config to an isolated temp path, and return the path. The
/// test selects this config by passing the path to
/// `CommandContext::with_config_path_override` (or `--config`), so it mutates no
/// process environment, acquires no `ENV_LOCK`, and runs in parallel.
///
/// # Panics
///
/// Panics if the temp directory cannot be created.
#[expect(clippy::unwrap_used)]
pub async fn setup_isolated_env() -> (wiremock::MockServer, tempfile::TempDir, std::path::PathBuf) {
    let mock = wiremock::MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config_to(&tmp, &default_test_config(&mock.uri()));
    (mock, tmp, config_path)
}

/// Acquire `ENV_LOCK` and point `XDG_CONFIG_HOME` at an empty temp dir (no
/// config written). Use this to exercise paths that must fail *before* config
/// load / network connect: a missing config makes any `connect_and_configure`
/// fail, so a test that still expects a local validation/IO error proves that
/// error wins the ordering race.
///
/// # Panics
///
/// Panics if the temp directory cannot be created.
#[expect(clippy::unwrap_used)]
pub async fn setup_empty_config_env() -> (tokio::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    let lock = super::ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests that mutate process environment hold ENV_LOCK.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
    (lock, tmp)
}

// ── Config command test support ──────────────────────────────────────
//
// Shared by the per-leaf `config/*_tests.rs` siblings. Config commands are
// local-only (no network client), so these helpers never start a mock
// server; they drive `config::execute` against an XDG root pointed at a
// temp dir by `setup_empty_config_env`.

/// Path to the config file under the active XDG test root.
///
/// # Panics
///
/// Panics if the config path cannot be resolved.
#[expect(clippy::unwrap_used)]
pub fn config_path() -> std::path::PathBuf {
    crate::config::Config::path_at(None).unwrap()
}

/// Load and validate the config under the active test root.
///
/// # Panics
///
/// Panics if the config cannot be loaded or fails validation.
#[expect(clippy::unwrap_used)]
pub fn load_config() -> crate::config::Config {
    crate::config::Config::load_at(Some(&config_path())).unwrap()
}

/// Load the config *without* validation — for asserting on-disk states the
/// validator rejects, such as the credential-less server `unset-keyring`
/// leaves behind.
///
/// # Panics
///
/// Panics if the config file cannot be read or parsed.
#[expect(clippy::unwrap_used)]
pub fn load_config_unvalidated() -> crate::config::Config {
    let content = std::fs::read_to_string(config_path()).unwrap();
    toml::from_str(&content).unwrap()
}

/// Apply a validated mutation to the config under the active test root.
pub fn update_config(
    mutator: impl FnOnce(&mut crate::config::Config) -> crate::error::Result<()>,
) -> crate::error::Result<crate::config::Config> {
    crate::config::Config::update_locked_at(Some(&config_path()), mutator)
}

/// Apply a mutation *without* whole-config validation.
pub fn update_config_without_validation(
    mutator: impl FnOnce(&mut crate::config::Config) -> crate::error::Result<()>,
) -> crate::error::Result<crate::config::Config> {
    crate::config::Config::update_locked_without_validation_at(Some(&config_path()), mutator)
}

/// Seed an inline-API-key server via `config set-server`.
///
/// # Panics
///
/// Panics if the `set-server` command returns an error.
#[expect(clippy::unwrap_used)]
pub async fn seed_inline_server(name: &str, url: &str, api_key: &str) {
    let mut io = CapturedIo::new();
    crate::commands::config::execute(
        &crate::cli::ConfigAction::SetServer {
            name: name.into(),
            url: url.into(),
            api_key: Some(api_key.into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::output::OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await
    .unwrap();
}

/// Run a `ConfigAction`, capture stdout, and parse it as JSON.
///
/// Gated to `cfg(test)`: it takes the crate-internal `ConfigAction`, so only
/// in-crate unit tests can call it (integration tests drive config through
/// `dispatch`).
///
/// # Panics
///
/// Panics if the command errors or its stdout is not valid JSON.
#[cfg(test)]
#[expect(clippy::unwrap_used)]
pub(crate) async fn run_config_action_json(action: crate::cli::ConfigAction) -> serde_json::Value {
    let mut io = CapturedIo::new();
    crate::commands::config::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::output::OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await
    .unwrap();
    serde_json::from_str(io.out_str().trim()).unwrap()
}

/// Seed a keyring-backed secret for `server` by priming the
/// `BZR_KEYRING_TEST_SECRET` test hook and running `set-keyring`. The caller
/// must already hold `ENV_LOCK` (via `setup_empty_config_env`) and have
/// installed the test store with `keyring::install_test_store()`.
///
/// # Panics
///
/// Panics if the `set-keyring` command returns an error.
#[cfg(feature = "keyring")]
#[expect(clippy::unwrap_used)]
pub async fn seed_keyring_secret(server: &str, secret: &str) {
    let mut io = CapturedIo::new();
    // SAFETY: Serialized via ENV_LOCK held by the caller's setup.
    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", secret) };
    crate::commands::config::execute(
        &crate::cli::ConfigAction::SetKeyring {
            name: server.into(),
            service: None,
            account: None,
        },
        &crate::commands::runtime::context::CommandContext::new(
            None,
            crate::types::output::OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via ENV_LOCK held by the caller's setup.
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };
}

/// Build a full-shaped attachment fixture with common test defaults.
pub fn make_attachment(
    id: u64,
    bug_id: u64,
    file_name: &str,
    summary: &str,
    data: Option<String>,
) -> crate::types::attachment::Attachment {
    crate::types::attachment::Attachment {
        id,
        bug_id,
        file_name: file_name.into(),
        summary: summary.into(),
        content_type: "text/plain".into(),
        creator: Some("author@example.com".into()),
        creation_time: Some("2025-03-01T09:00:00Z".into()),
        last_change_time: Some("2025-03-02T10:00:00Z".into()),
        size: 1234,
        is_obsolete: false,
        is_private: false,
        is_patch: false,
        flags: Vec::new(),
        data,
    }
}

/// Owned `Vec<u8>` buffers for capturing stdout and stderr in tests, plus
/// a `Writers` constructor that borrows them.
#[non_exhaustive]
pub struct CapturedIo {
    pub out: Vec<u8>,
    pub err: Vec<u8>,
}

impl CapturedIo {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// Construct a `Writers` borrowing this buffer pair.
    pub fn writers(&mut self) -> crate::output::writers::Writers<'_> {
        crate::output::writers::Writers::new(&mut self.out, &mut self.err)
    }

    /// View captured stdout as `&str` (non-UTF-8 bytes are replaced with the
    /// empty string — tests should hold UTF-8 invariants and this method
    /// makes assertion failures more readable than working in raw bytes).
    pub fn out_str(&self) -> &str {
        std::str::from_utf8(&self.out).unwrap_or("")
    }

    /// View captured stderr as `&str`. Same caveat as `out_str`.
    pub fn err_str(&self) -> &str {
        std::str::from_utf8(&self.err).unwrap_or("")
    }
}

impl Default for CapturedIo {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BooleanChartTriple {
    field: String,
    operator: String,
    value: String,
}

impl BooleanChartTriple {
    fn new(field: &str, operator: &str, value: &str) -> Self {
        Self {
            field: field.to_string(),
            operator: operator.to_string(),
            value: value.to_string(),
        }
    }
}

#[derive(Default)]
struct BooleanChartParts {
    field: Option<String>,
    operator: Option<String>,
    value: Option<String>,
}

pub struct HasBooleanChartTriples {
    expected: BTreeSet<BooleanChartTriple>,
}

impl HasBooleanChartTriples {
    pub fn new(expected: &[(&str, &str, &str)]) -> Self {
        Self {
            expected: expected
                .iter()
                .map(|(field, operator, value)| BooleanChartTriple::new(field, operator, value))
                .collect(),
        }
    }
}

impl wiremock::Match for HasBooleanChartTriples {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let actual = boolean_chart_triples(request);
        self.expected.iter().all(|triple| actual.contains(triple))
    }
}

fn boolean_chart_triples(request: &wiremock::Request) -> BTreeSet<BooleanChartTriple> {
    let mut parts_by_index = BTreeMap::<String, BooleanChartParts>::new();
    for (key, value) in request.url.query_pairs() {
        let Some((part, index)) = boolean_chart_key_part(&key) else {
            continue;
        };
        let parts = parts_by_index.entry(index.to_string()).or_default();
        match part {
            'f' => parts.field = Some(value.into_owned()),
            'o' => parts.operator = Some(value.into_owned()),
            'v' => parts.value = Some(value.into_owned()),
            _ => {}
        }
    }

    parts_by_index
        .into_values()
        .filter_map(|parts| {
            Some(BooleanChartTriple {
                field: parts.field?,
                operator: parts.operator?,
                value: parts.value?,
            })
        })
        .collect()
}

fn boolean_chart_key_part(key: &str) -> Option<(char, &str)> {
    if let Some(index) = key.strip_prefix('f') {
        return valid_boolean_chart_index(index).map(|index| ('f', index));
    }
    if let Some(index) = key.strip_prefix('o') {
        return valid_boolean_chart_index(index).map(|index| ('o', index));
    }
    if let Some(index) = key.strip_prefix('v') {
        return valid_boolean_chart_index(index).map(|index| ('v', index));
    }
    None
}

fn valid_boolean_chart_index(index: &str) -> Option<&str> {
    if !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
        Some(index)
    } else {
        None
    }
}

/// Build a mock XML-RPC Bug.search response containing one bug.
pub fn xmlrpc_bug_response(id: i64, summary: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse><params><param><value><struct>
              <member><name>bugs</name><value><array><data>
                <value><struct>
                  <member><name>id</name><value><int>{id}</int></value></member>
                  <member><name>summary</name><value><string>{summary}</string></value></member>
                  <member><name>status</name><value><string>NEW</string></value></member>
                  <member><name>product</name><value><string>TestProduct</string></value></member>
                  <member><name>component</name><value><string>General</string></value></member>
                  <member><name>assigned_to</name><value><string>dev@example.com</string></value></member>
                  <member><name>priority</name><value><string>P1</string></value></member>
                  <member><name>severity</name><value><string>normal</string></value></member>
                  <member><name>keywords</name><value><array><data></data></array></value></member>
                  <member><name>blocks</name><value><array><data></data></array></value></member>
                  <member><name>depends_on</name><value><array><data></data></array></value></member>
                  <member><name>cc</name><value><array><data></data></array></value></member>
                </struct></value>
              </data></array></value></member>
            </struct></value></param></params></methodResponse>"#
    )
}

#[cfg(test)]
#[path = "test_helpers_tests.rs"]
mod tests;
