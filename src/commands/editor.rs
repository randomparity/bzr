//! Generic `$EDITOR` launcher used by interactive composition flows
//! (`bzr comment add`, `bzr bug create`).
//!
//! This module is domain-agnostic: it manages a tempfile lifecycle
//! and spawns `$EDITOR` (or `vi` fallback). Callers own the buffer
//! template and any post-edit parsing/filtering rules.

use std::io::Write;
use std::path::PathBuf;

use crate::error::{BzrError, Result};

/// RAII tempfile that removes itself on drop. Failures during cleanup
/// are debug-logged, not propagated.
pub(super) struct TempFile {
    pub(super) path: PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::debug!(path = %self.path.display(), "failed to remove temp file: {e}");
        }
    }
}

/// Create a tempfile in the system temp dir, write `initial_content`
/// into it, and return a handle that will clean up on drop. The
/// filename is `bzr-<prefix>-<pid>.txt`.
pub(super) fn create_tempfile(prefix: &str, initial_content: &str) -> Result<TempFile> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("bzr-{prefix}-{}.txt", std::process::id()));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(initial_content.as_bytes())?;
    drop(file);
    Ok(TempFile { path })
}

/// Spawn `$EDITOR` (or `vi` fallback) on a freshly created tempfile
/// pre-populated with `initial`. Returns the post-edit file contents
/// as a UTF-8 string.
///
/// Errors:
/// - `BzrError::Io` if the tempfile cannot be created or read.
/// - `BzrError::InputValidation` if `$EDITOR` exits non-zero.
pub(super) fn launch(initial: &str, prefix: &str) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let tmpfile = create_tempfile(prefix, initial)?;

    let status = std::process::Command::new(&editor)
        .arg(&tmpfile.path)
        .status()?;

    if !status.success() {
        return Err(BzrError::InputValidation(format!(
            "{editor} exited with error"
        )));
    }

    let content = std::fs::read_to_string(&tmpfile.path)?;
    Ok(content)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
