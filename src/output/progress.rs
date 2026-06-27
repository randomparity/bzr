//! Structured progress events (`--progress ndjson`). Each emit function is a
//! no-op when the mode is `None`; otherwise it writes one compact JSON line to
//! the supplied stderr writer. Events are discriminated by the `event` key.

use std::io::Write;

use serde::Serialize;

use crate::types::ProgressFormat;

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum ProgressEvent<'a> {
    Page {
        n: u32,
        fetched: usize,
    },
    Batch {
        n: usize,
        total: usize,
        ok: usize,
        failed: usize,
    },
    Done {
        fetched: usize,
    },
    Error {
        error_type: &'a str,
        exit_code: i32,
    },
}

fn emit(format: Option<ProgressFormat>, err: &mut dyn Write, event: &ProgressEvent<'_>) {
    // Only `ndjson` exists; the `Some` check is the enable gate. When another
    // format is added, branch on the variant here.
    if format.is_none() {
        return;
    }
    if let Ok(line) = serde_json::to_string(event) {
        let _ = writeln!(err, "{line}");
    }
}

pub(crate) fn page_event(
    format: Option<ProgressFormat>,
    err: &mut dyn Write,
    n: u32,
    fetched: usize,
) {
    emit(format, err, &ProgressEvent::Page { n, fetched });
}

/// Cumulative counters for a `batch` progress event: the 1-based index of the
/// item just attempted, the batch size, and the running ok/failed tallies.
pub(crate) struct BatchProgress {
    pub n: usize,
    pub total: usize,
    pub ok: usize,
    pub failed: usize,
}

pub(crate) fn batch_event(format: Option<ProgressFormat>, err: &mut dyn Write, p: &BatchProgress) {
    emit(
        format,
        err,
        &ProgressEvent::Batch {
            n: p.n,
            total: p.total,
            ok: p.ok,
            failed: p.failed,
        },
    );
}

pub(crate) fn done_event(format: Option<ProgressFormat>, err: &mut dyn Write, fetched: usize) {
    emit(format, err, &ProgressEvent::Done { fetched });
}

pub fn error_event(
    format: Option<ProgressFormat>,
    err: &mut dyn Write,
    error_type: &str,
    exit_code: i32,
) {
    emit(
        format,
        err,
        &ProgressEvent::Error {
            error_type,
            exit_code,
        },
    );
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
