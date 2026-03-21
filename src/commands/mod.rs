pub mod attachment;
pub mod bug;
pub mod classification;
pub mod comment;
pub mod component;
pub mod config_cmd;
pub mod field;
mod flags;
pub mod group;
pub mod product;
pub mod server;
mod shared;
pub mod user;
pub mod whoami;

/// Shared test utilities for command module tests.
/// Tests that set `XDG_CONFIG_HOME` must hold this lock to avoid races.
#[cfg(test)]
pub(crate) mod test_helpers {
    pub(crate) use crate::ENV_LOCK;

    /// Acquire ENV_LOCK, start a mock server, create a temp dir, and configure it.
    /// Returns the guard, mock server, and temp dir (all must stay alive for the test).
    #[expect(clippy::unwrap_used)]
    pub async fn setup_test_env() -> (
        tokio::sync::MutexGuard<'static, ()>,
        wiremock::MockServer,
        tempfile::TempDir,
    ) {
        let lock = ENV_LOCK.lock().await;
        let mock = wiremock::MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());
        (lock, mock, tmp)
    }

    #[expect(clippy::unwrap_used)]
    pub fn setup_config(tmp: &tempfile::TempDir, server_url: &str) {
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(config_dir.clone()).unwrap();
        let config_content = format!(
            r#"
default_server = "test"

[servers.test]
url = "{server_url}"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
"#,
        );
        std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
    }
}
