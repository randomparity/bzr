//! Process-global `--dry-run` switch.
//!
//! `--dry-run` is a global CLI flag. Like the network-tuning globals (see
//! `lib::apply_network_tuning`), it is installed once per process by
//! `dispatch` and read by the bug mutation handlers, which short-circuit
//! before any write API call and emit a `"action":"dry-run"` payload instead.
//!
//! Using a process-global avoids threading a `dry_run` bool through every
//! command handler signature (and every test call site). The CLI runs a
//! single dispatch per process, so the shared state is not a concurrency
//! concern in practice; tests reset it to `false` via `setup_test_env`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Install the dry-run state from the global `--dry-run` flag.
///
/// Production calls this once, from `dispatch`. Test-side callers must hold
/// `ENV_LOCK` (as `setup_test_env` does) so they serialize with the mutation
/// tests; otherwise a parallel test could observe a foreign value.
pub fn set(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether the current invocation is a dry run (no writes).
///
/// Test-side callers that toggle this via [`set`] must hold `ENV_LOCK` so they
/// serialize with the mutation tests (which reset it through `setup_test_env`);
/// otherwise a parallel test could observe a foreign value. Behavior is covered
/// end-to-end by the dispatch and per-handler dry-run tests.
#[must_use]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
