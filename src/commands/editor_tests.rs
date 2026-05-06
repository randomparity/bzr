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
fn launch_returns_post_edit_content_via_fake_editor() {
    let script_dir = std::env::temp_dir();
    let script = script_dir.join(format!("bzr-fake-editor-{}.sh", std::process::id()));
    std::fs::write(&script, "#!/bin/sh\nprintf 'edited content\\n' > \"$1\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev = std::env::var("EDITOR").ok();
    // SAFETY: cargo test runs unit tests on a single process; the
    // env var is restored at the end of this test.
    unsafe { std::env::set_var("EDITOR", &script) };

    let result = super::launch("initial body\n", "test-launch").unwrap();
    assert_eq!(result, "edited content\n");

    // SAFETY: see set_var above.
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
fn launch_propagates_editor_failure_as_input_validation() {
    let script_dir = std::env::temp_dir();
    let script = script_dir.join(format!("bzr-fail-editor-{}.sh", std::process::id()));
    std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev = std::env::var("EDITOR").ok();
    // SAFETY: see other set_var sites in this file.
    unsafe { std::env::set_var("EDITOR", &script) };

    let err = super::launch("initial\n", "test-fail").unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(m) if m.contains("exited with error")),
        "got {err:?}"
    );

    // SAFETY: see other set_var sites.
    unsafe {
        if let Some(p) = prev {
            std::env::set_var("EDITOR", p);
        } else {
            std::env::remove_var("EDITOR");
        }
    }
    let _ = std::fs::remove_file(&script);
}
