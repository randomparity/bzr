//! Generic `$EDITOR` launcher used by interactive composition flows
//! (`bzr comment add`, `bzr bug create`).
//!
//! This module is domain-agnostic: it manages a tempfile lifecycle
//! and spawns `$EDITOR` (or `vi` fallback). Callers own the buffer
//! template and any post-edit parsing/filtering rules.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{io_with_context, BzrError, Result};

const MAX_TEMPFILE_ATTEMPTS: u64 = 1_000;
static TEMPFILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Create a unique tempfile in the system temp dir, write `initial_content`
/// into it, and return a handle that will clean up on drop.
pub(super) fn create_tempfile(prefix: &str, initial_content: &str) -> Result<TempFile> {
    let dir = std::env::temp_dir();
    let (path, mut file) = create_unique_editor_file(&dir, prefix)?;
    file.write_all(initial_content.as_bytes()).map_err(|e| {
        let _ = std::fs::remove_file(&path);
        io_with_context(
            format!("failed to write editor temp file '{}'", path.display()),
            &e,
        )
    })?;
    drop(file);
    Ok(TempFile { path })
}

fn create_unique_editor_file(
    dir: &std::path::Path,
    prefix: &str,
) -> Result<(PathBuf, std::fs::File)> {
    for _ in 0..MAX_TEMPFILE_ATTEMPTS {
        let path = editor_tempfile_path(dir, prefix);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(io_with_context(
                    format!("failed to create editor temp file '{}'", path.display()),
                    &e,
                ));
            }
        }
    }
    Err(BzrError::config(format!(
        "could not create a unique editor temp file in '{}'",
        dir.display()
    )))
}

fn editor_tempfile_path(dir: &std::path::Path, prefix: &str) -> PathBuf {
    let counter = TEMPFILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("bzr-{prefix}-{}-{counter}.txt", std::process::id()))
}

/// Spawn `$EDITOR` (or `vi` fallback) on a freshly created tempfile
/// pre-populated with `initial`. Returns the post-edit file contents
/// as a UTF-8 string.
///
/// Errors:
/// - `BzrError::Io` if the tempfile cannot be created or read.
/// - `BzrError::InputValidation` if `$EDITOR` exits non-zero.
pub(crate) fn launch(initial: &str, prefix: &str) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let tmpfile = create_tempfile(prefix, initial)?;

    let status = std::process::Command::new(&editor)
        .arg(&tmpfile.path)
        .status()
        .map_err(|e| {
            io_with_context(
                format!(
                    "failed to launch editor '{editor}' for '{}'",
                    tmpfile.path.display()
                ),
                &e,
            )
        })?;

    if !status.success() {
        return Err(BzrError::InputValidation(format!(
            "{editor} exited with error while editing '{}'",
            tmpfile.path.display()
        )));
    }

    let content = std::fs::read_to_string(&tmpfile.path).map_err(|e| {
        io_with_context(
            format!(
                "failed to read editor temp file '{}'",
                tmpfile.path.display()
            ),
            &e,
        )
    })?;
    Ok(content)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
