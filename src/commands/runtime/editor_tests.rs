#![expect(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[test]
fn tempfile_writes_initial_content_and_cleans_up_on_drop() {
    let path: PathBuf;
    {
        let tf = super::create_tempfile("test-tempfile", "hello\n").unwrap();
        path = tf.path.clone();
        let content = std::fs::read_to_string(&tf.path).unwrap();
        assert_eq!(content, "hello\n");
    }
    assert!(!path.exists(), "tempfile should be removed on drop");
}

#[test]
fn tempfile_create_error_names_temp_path() {
    let result = super::create_tempfile("missing-parent/test-tempfile", "hello\n");
    assert!(result.is_err(), "expected tempfile creation to fail");
    let err = result.err().unwrap().to_string();

    assert!(
        err.contains("create editor temp file") && err.contains("missing-parent"),
        "error should name temp file path, got: {err}"
    );
}

#[test]
fn launch_returns_post_edit_content_via_fake_editor() {
    let _guard = crate::ENV_LOCK.blocking_lock();
    let script_dir = std::env::temp_dir();
    let script = script_dir.join(format!("bzr-fake-editor-{}.sh", std::process::id()));
    std::fs::write(&script, "#!/bin/sh\nprintf 'edited content\\n' > \"$1\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev = std::env::var("EDITOR").ok();
    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe { std::env::set_var("EDITOR", &script) };

    let result = super::launch("initial body\n", "test-launch").unwrap();
    assert_eq!(result, "edited content\n");

    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let _ = std::fs::remove_file(&script);
}

#[test]
fn launch_error_names_editor_command() {
    let _guard = crate::ENV_LOCK.blocking_lock();
    let missing_editor =
        std::env::temp_dir().join(format!("bzr-missing-editor-{}", std::process::id()));
    let prev = std::env::var("EDITOR").ok();
    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe { std::env::set_var("EDITOR", &missing_editor) };

    let err = super::launch("initial\n", "test-missing-editor")
        .unwrap_err()
        .to_string();

    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let missing_editor = missing_editor.to_string_lossy();

    assert!(
        err.contains("launch editor") && err.contains(missing_editor.as_ref()),
        "error should name editor command, got: {err}"
    );
}

#[test]
fn launch_propagates_editor_failure_as_input_validation() {
    let _guard = crate::ENV_LOCK.blocking_lock();
    let script_dir = std::env::temp_dir();
    let script = script_dir.join(format!("bzr-fail-editor-{}.sh", std::process::id()));
    std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev = std::env::var("EDITOR").ok();
    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe { std::env::set_var("EDITOR", &script) };

    let err = super::launch("initial\n", "test-fail").unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(m) if m.contains("exited with error")),
        "got {err:?}"
    );

    // SAFETY: ENV_LOCK guarantees no other thread is reading or writing the environment for the lifetime of `_guard`.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let _ = std::fs::remove_file(&script);
}
