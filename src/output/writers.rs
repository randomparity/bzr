//! Writer plumbing for command-layer output.
//!
//! The two streams are bundled into a single `Writers` parameter so command
//! signatures stay compact (one parameter, not two). Inside the struct the
//! streams are `&mut dyn Write` — the command layer doesn't need to care
//! what concrete writer backs each stream. `main.rs` constructs `Writers`
//! from locked `io::stdout()` / `io::stderr()`; tests use `CapturedIo` to
//! own `Vec<u8>` buffers and borrow them through `Writers`.

use std::ffi::OsStr;
use std::io::Write;

/// An optional width resolved for the process's real stdout destination.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TableWidth(Option<usize>);

/// Resolve an explicit table-width override before a detected stdout width.
pub fn resolve_table_width(explicit: Option<&OsStr>, detected: Option<u16>) -> TableWidth {
    let explicit = explicit.and_then(|value| {
        if let Some(value) = value.to_str() {
            match value.parse::<u16>() {
                Ok(width) if width > 0 => Some(usize::from(width)),
                _ => {
                    tracing::warn!("invalid BZR_TABLE_WIDTH; ignoring it");
                    None
                }
            }
        } else {
            tracing::warn!("invalid BZR_TABLE_WIDTH; ignoring it");
            None
        }
    });

    TableWidth(explicit.or_else(|| detected.filter(|&width| width > 0).map(usize::from)))
}

/// Detect the current stdout width where the platform supports handle-specific probing.
#[cfg(any(unix, windows))]
pub fn detected_stdout_width() -> Option<u16> {
    terminal_size::terminal_size_of(std::io::stdout()).map(|(terminal_size::Width(width), _)| width)
}

/// Platforms without a handle-specific terminal-size implementation stay unbounded.
#[cfg(not(any(unix, windows)))]
pub fn detected_stdout_width() -> Option<u16> {
    None
}

#[non_exhaustive]
pub struct Writers<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    table_width: TableWidth,
}

impl<'a> Writers<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self::with_table_width(out, err, TableWidth::default())
    }

    pub fn with_table_width(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        table_width: TableWidth,
    ) -> Self {
        Self {
            out,
            err,
            table_width,
        }
    }

    pub fn table_width(&self) -> Option<usize> {
        self.table_width.0
    }
}

#[cfg(test)]
#[path = "writers_tests.rs"]
mod tests;
