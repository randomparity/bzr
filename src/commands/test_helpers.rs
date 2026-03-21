/// Shared test utilities for command module tests.
/// Tests that set `XDG_CONFIG_HOME` must hold this lock to avoid races.
pub(super) use crate::ENV_LOCK;

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

/// Capture stdout written during an async operation.
///
/// Redirects file descriptor 1 to a temp file, runs the future, restores
/// stdout, then returns the captured content. Must be called while holding
/// `ENV_LOCK` (tests are single-threaded via `setup_test_env`).
#[cfg(unix)]
#[expect(clippy::unwrap_used)]
pub async fn capture_stdout<F, T>(f: F) -> (T, String)
where
    F: std::future::Future<Output = T>,
{
    use std::io::{Read, Seek, Write};
    use std::os::unix::io::AsRawFd;

    extern "C" {
        fn dup(fd: std::ffi::c_int) -> std::ffi::c_int;
        fn dup2(oldfd: std::ffi::c_int, newfd: std::ffi::c_int) -> std::ffi::c_int;
        fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
    }

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let tmp_fd = tmp.as_file().as_raw_fd();

    // SAFETY: dup() on a valid fd is safe; tests are serialized via ENV_LOCK.
    let saved_stdout = unsafe { dup(1) };
    assert!(saved_stdout >= 0, "dup(1) failed");

    // SAFETY: dup2() on valid fds is safe.
    unsafe {
        dup2(tmp_fd, 1);
    }

    let result = f.await;
    std::io::stdout().flush().unwrap();

    // SAFETY: Restoring the saved fd.
    unsafe {
        dup2(saved_stdout, 1);
        close(saved_stdout);
    }

    let mut captured = String::new();
    let mut file = tmp.reopen().unwrap();
    file.seek(std::io::SeekFrom::Start(0)).unwrap();
    file.read_to_string(&mut captured).unwrap();

    (result, captured)
}

/// Extract the first valid JSON value from a string that may contain
/// other test output mixed in (due to concurrent test threads writing
/// to the same stdout fd).
pub fn extract_json(output: &str) -> serde_json::Value {
    // Try parsing the full output first (common case).
    if let Ok(v) = serde_json::from_str(output) {
        return v;
    }
    // Find first `[` or `{` and try parsing from there.
    for (i, ch) in output.char_indices() {
        if ch == '[' || ch == '{' {
            if let Ok(v) = serde_json::from_str(&output[i..]) {
                return v;
            }
            // Try to find the matching close bracket by attempting
            // progressively shorter substrings from the end.
            let rest = &output[i..];
            for (j, jch) in rest.char_indices().rev() {
                let closing = if ch == '[' { ']' } else { '}' };
                if jch == closing {
                    if let Ok(v) = serde_json::from_str(&rest[..=j]) {
                        return v;
                    }
                }
            }
        }
    }
    panic!("no valid JSON found in captured output: {output}");
}
