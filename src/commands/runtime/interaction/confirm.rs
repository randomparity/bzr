//! Interactive confirmation gate for large batch mutations.
//!
//! A mistyped ID list or wrong filter can mass-mutate bugs irreversibly, so a
//! batch larger than [`BATCH_THRESHOLD`] prompts for confirmation at an
//! interactive TTY. `--yes`/`-y` bypasses the prompt, and non-interactive runs
//! (piped stdin, agents) auto-bypass so they are never blocked.
//!
//! The parsed `--yes` value is carried on
//! [`crate::commands::runtime::invocation::CommandContext`], so command handlers
//! can make prompt decisions without consulting process-global state.

use std::io::{BufRead, Write};

use crate::error::Result;
use crate::output::writers::Writers;

/// Batches strictly larger than this prompt for confirmation at a TTY.
pub const BATCH_THRESHOLD: usize = 10;

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
        return Err(crate::error::BzrError::input(format!(
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

/// Prompt for confirmation before a large batch mutation, wiring the real
/// stdin/TTY into [`should_prompt`] and [`read_yes_no`]. The `should_prompt`
/// gate is checked first, so stdin is locked only when a prompt is actually
/// shown. Returns whether to proceed. Shared by every command that fans a
/// single mutation across a user-supplied ID list (`bug update` and its
/// convenience verbs, `attachment upload`).
pub fn confirm_batch(count: usize, assume_yes: bool, w: &mut Writers<'_>) -> Result<bool> {
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();
    if !should_prompt(count, assume_yes, is_tty) {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    read_yes_no(&mut stdin.lock(), w.err, count)
}

#[cfg(test)]
#[path = "confirm_tests.rs"]
mod tests;
