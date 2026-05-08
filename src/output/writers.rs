//! Writer plumbing for command-layer output.
//!
//! The two streams are bundled into a single `Writers` parameter so command
//! signatures stay compact (one parameter, not two). Inside the struct the
//! streams are `&mut dyn Write` — the command layer doesn't need to care
//! what concrete writer backs each stream. `main.rs` constructs `Writers`
//! from locked `io::stdout()` / `io::stderr()`; tests use `CapturedIo` to
//! own `Vec<u8>` buffers and borrow them through `Writers`.

use std::io::Write;

#[non_exhaustive]
pub struct Writers<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

impl<'a> Writers<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self { out, err }
    }
}

#[cfg(test)]
#[path = "writers_tests.rs"]
mod tests;
