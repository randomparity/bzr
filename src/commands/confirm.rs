//! Interactive confirmation gate for large batch mutations.
//!
//! A mistyped ID list or wrong filter can mass-mutate bugs irreversibly, so a
//! batch larger than [`BATCH_THRESHOLD`] prompts for confirmation at an
//! interactive TTY. `--yes`/`-y` bypasses the prompt, and non-interactive runs
//! (piped stdin, agents) auto-bypass so they are never blocked.
//!
//! `--yes` is a global flag installed once per process by `dispatch` (the same
//! pattern as `--dry-run` and the network-tuning globals), so it does not need
//! threading through every handler signature. See [`crate::commands::dry_run`].

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::Result;

/// Batches strictly larger than this prompt for confirmation at a TTY.
pub const BATCH_THRESHOLD: usize = 10;

static ASSUME_YES: AtomicBool = AtomicBool::new(false);

/// Install the assume-yes state from the global `--yes` flag.
///
/// Production calls this once, from `dispatch`. Test-side callers must hold
/// `ENV_LOCK` (as `setup_test_env` does) so they serialize with the mutation
/// tests; otherwise a parallel test could observe a foreign value.
pub fn set_yes(assume_yes: bool) {
    ASSUME_YES.store(assume_yes, Ordering::Relaxed);
}

/// Whether `--yes` was given (skip all confirmation prompts).
#[must_use]
pub fn yes() -> bool {
    ASSUME_YES.load(Ordering::Relaxed)
}

/// Whether a batch of `count` items needs an interactive confirmation prompt.
/// A prompt is needed only above the threshold, when `--yes` was not given, and
/// only at an interactive TTY — so piped/non-interactive runs never block.
#[must_use]
pub fn should_prompt(count: usize, assume_yes: bool, is_tty: bool) -> bool {
    count > BATCH_THRESHOLD && !assume_yes && is_tty
}

/// Render the prompt to `w` and read a yes/no answer from `reader`. Anything
/// other than `y`/`yes` (case-insensitive, trimmed) is a no — the safe default,
/// so a bare Enter declines.
///
/// An immediate EOF (no line at all) is **not** a silent decline: it means no
/// answer could be read — typically because stdin was already consumed by a
/// `--comment -` / `--comment-file -` body on the same command. That returns an
/// error naming `--yes`, so the user gets an actionable message instead of a
/// confusing "aborted" with no input typed.
pub fn read_yes_no<R: BufRead, W: Write + ?Sized>(
    reader: &mut R,
    w: &mut W,
    count: usize,
) -> Result<bool> {
    let _ = write!(
        w,
        "About to modify {count} bugs; this cannot be undone. Continue? [y/N] "
    );
    let _ = w.flush();
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(crate::error::BzrError::InputValidation(format!(
            "could not read a confirmation answer from stdin (it may have been \
             consumed by --comment - / --comment-file -); re-run with --yes to \
             confirm modifying {count} bugs"
        )));
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
#[path = "confirm_tests.rs"]
mod tests;
