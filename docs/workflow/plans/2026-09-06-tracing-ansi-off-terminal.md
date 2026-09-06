# Plan: plain-text tracing off a terminal, and a fresh comparison container

Goal: bzr must not write ANSI escapes to a stderr that is not a terminal, so the comparison
harness observes transport with no caller-supplied environment variable, and the comparison tier
must run against a container created for that run.

Architecture: `src/main.rs` gains one pure helper deciding the tracing stream's ANSI setting and
passes it to the `tracing_subscriber::fmt()` builder; `src/main_tests.rs` covers the helper. The
functional tier gains one test in `tests/functional/phases/17-global-options.sh`, the phase that
already owns global-flag behaviour. `Makefile` and `tests/functional/run-compare-all.sh` swap
`setup-bugzilla.sh start` for the existing `setup-bugzilla.sh reset`. `docs/bzr-cli.md` gains one
corrected table row.

Tech stack: Rust 2021, `tracing-subscriber` `=0.3.23` (`env-filter`), Bash for the harness, GNU
Make. Spec: `docs/workflow/specs/2026-09-06-tracing-ansi-off-terminal-design.md`. ADR:
`docs/adr/0058-suppress-tracing-ansi-off-terminal.md`. Branch:
`feat/suppress-tracing-ansi-on-non-tty-722`; `BASE_BRANCH` = `main`.

Expected implementation size: 80–110 changed lines (S) — derived from the tasks below: one helper
plus one builder call in `src/main.rs`, six unit cases in `src/main_tests.rs`, one functional
test, one docs table row, and three lines across `Makefile` and `run-compare-all.sh`.

## Global Constraints

- MSRV `1.89` (`Cargo.toml` `rust-version`). `Option::is_none_or` (1.82) and `OsStr::is_empty`
  (1.9) are both available.
- Clippy pedantic, `unwrap_used` denied; `make lint` runs
  `cargo clippy --all-targets --features test-helpers -- -D warnings`.
- Unit tests live in sibling `<name>_tests.rs` files; `src/main_tests.rs` exists, starts with
  `#![expect(clippy::expect_used)]`, and opens with `use super::*;`. Inline `mod tests { … }` in
  `src/` is forbidden and `make check-test-layout` enforces it.
- `tests/functional/phases/*.sh` are shellcheck'd (`make check-shell`) but not shfmt'd, and use
  4-space indent. Test ids must match `^[a-z0-9]+(-[a-z0-9]+)*$` and be unique
  (`make check-functional-test-ids`).
- Run guardrails bare — no pipe to `tail`, no `|| true`. `make test` can exceed a two-minute tool
  timeout; background it. `make test-one T=<substr>` while iterating.
- Do not edit `src/client/auth/`, `src/client/transport.rs`,
  `tests/functional/phases/08c-bugs-create-fields.sh`, `tests/functional/pybz/container-tests.sh`,
  or `tests/functional/redhat-shape-proxy.py`; concurrent issues own them.

## Task 1 — bzr writes plain text to a non-terminal stderr

**Interfaces.** Defines `tracing_ansi_enabled` (signature in step 3) in `src/main.rs`, private to
that binary crate, consumed by `main` and by `src/main_tests.rs` through its existing
`use super::*;`. Consumes `Cli::no_color` (`src/cli/mod.rs:253`, `pub no_color: bool`),
`std::io::IsTerminal` (already imported at `src/main.rs:1`), and
`tracing_subscriber::fmt::SubscriberBuilder::with_ansi` (0.3.23, `src/fmt/mod.rs:633`,
`pub fn with_ansi(self, ansi: bool) -> SubscriberBuilder<…>`).

**Verification.**

- Contract: the tracing stream carries no ANSI when stderr is not a terminal, when `--no-color`
  is passed, or when `NO_COLOR` is non-empty; and carries ANSI otherwise.
  `Mode: focused-test` — `src/main_tests.rs`, the six cases in step 1. Expected red: they do not
  compile, `cannot find function `tracing_ansi_enabled``. Green: `make test-one T=tracing_ansi`.
- Contract: `docs/bzr-cli.md`'s `--no-color` row describes the behaviour bzr now has.
  `Mode: task-test-not-applicable` — a one-row prose change in a hand-written reference table
  with no executable consumer; a test grepping for the sentence would assert wording, not
  behaviour. The behaviour it describes is covered by the cases above.

**Steps.**

1. Add to `src/main_tests.rs`, and add `use std::ffi::OsStr;` to its imports:

   ```rust
   #[test]
   fn tracing_ansi_enabled_on_a_terminal_without_overrides() {
       assert!(tracing_ansi_enabled(false, None, true));
   }

   #[test]
   fn tracing_ansi_enabled_treats_empty_no_color_as_unset() {
       assert!(tracing_ansi_enabled(false, Some(OsStr::new("")), true));
   }

   #[test]
   fn tracing_ansi_disabled_off_a_terminal() {
       assert!(!tracing_ansi_enabled(false, None, false));
   }

   #[test]
   fn tracing_ansi_disabled_by_the_no_color_flag() {
       assert!(!tracing_ansi_enabled(true, None, true));
   }

   #[test]
   fn tracing_ansi_disabled_by_a_non_empty_no_color_env() {
       assert!(!tracing_ansi_enabled(false, Some(OsStr::new("1")), true));
   }

   #[test]
   fn tracing_ansi_off_a_terminal_ignores_an_empty_no_color_env() {
       assert!(!tracing_ansi_enabled(false, Some(OsStr::new("")), false));
   }
   ```

2. Run `make test-one T=tracing_ansi`. Expect a compile error naming `tracing_ansi_enabled`.

3. Add the helper to `src/main.rs`, below `tracing_filter_directive`:

   ```rust
   /// Decide whether the tracing subscriber may write ANSI escapes to stderr.
   ///
   /// `tracing_subscriber::fmt` defaults ANSI on unless `NO_COLOR` holds a
   /// non-empty value, and performs no terminal detection. An explicit
   /// `with_ansi` call replaces that default outright, so this helper has to
   /// re-apply the `NO_COLOR` rule rather than inherit it.
   fn tracing_ansi_enabled(
       no_color_flag: bool,
       no_color_env: Option<&std::ffi::OsStr>,
       stderr_is_terminal: bool,
   ) -> bool {
       if no_color_flag || !stderr_is_terminal {
           return false;
       }
       no_color_env.is_none_or(std::ffi::OsStr::is_empty)
   }
   ```

4. In `main`, replace the four-line `tracing_subscriber::fmt()` chain
   (`.with_env_filter(filter)`, `.with_writer(std::io::stderr)`, `.init()`) with:

   ```rust
   let stderr_ansi = tracing_ansi_enabled(
       cli.no_color,
       std::env::var_os("NO_COLOR").as_deref(),
       std::io::stderr().is_terminal(),
   );

   tracing_subscriber::fmt()
       .with_env_filter(filter)
       .with_ansi(stderr_ansi)
       .with_writer(std::io::stderr)
       .init();
   ```

5. Run `make test-one T=tracing_ansi`. Expect all six cases to pass.

6. Update the `--no-color` row in `docs/bzr-cli.md`, currently
   `| `--no-color` | Disable colored output. Color is also suppressed automatically when stdout is not a TTY. |`,
   so it states that the flag covers stdout and the stderr diagnostic stream and that each stream
   is judged on its own terminal status. Keep it to the one table row.

7. Run `make lint` bare, then `make test` bare in the background. Expect exit 0 from both.

**Acceptance.** For a debug build,
`BZKEY=x ./target/debug/bzr -vv --server-url http://127.0.0.1:9 --server-api-key-env BZKEY bug view 1 2>capture`
leaves `grep -c $'\033' capture` printing `0`, while the same command with stderr on a terminal
still shows colour. `make lint` and `make test` are green.

## Task 2 — the functional tier proves it and bites on regression

**Interfaces.** Consumes `run_bzr_raw`, `assert_success`, `assert_stderr_not_contains`,
`test_begin` / `test_pass` / `test_fail` / `test_skip`, and `observe_bzr_transport`, all defined
in `tests/functional/lib.sh` and in scope for a sourced phase, plus `$BUG1`, set by `08-bugs.sh`
and used the same way by tests already in this file. Defines nothing.

**Verification.**

- Contract: a real `bzr -vv` invocation whose stderr is a file yields a stderr with no escape
  byte, from which `observe_bzr_transport` derives a transport.
  `Mode: focused-test` — `tests/functional/phases/17-global-options.sh`, test id
  `verbose-tracing-is-plain-on-redirected-stderr`. Expected red: with `.with_ansi(true)`
  hard-coded, `make functional-test` reports it FAIL. Green: `make functional-test` with Task 1
  in place.

**Steps.**

1. In `tests/functional/phases/17-global-options.sh`, immediately after the
   `verbose-response-body-diagnostics-redact-api-keys` test and its `unset` line, add at 4-space
   indent:

   ```bash
   test_begin "verbose-tracing-is-plain-on-redirected-stderr" "-vv tracing is plain text and its transport is observable"
   if [[ -n "$BUG1" ]]; then
       run_bzr_raw -vv bug view "$BUG1"
       if assert_success && assert_stderr_not_contains $'\033'; then
           if observe_bzr_transport; then test_pass
           else test_fail "transport is not observable from -vv stderr"; fi
       fi
   else test_skip "no BUG1"; fi
   ```

2. Run `make check-shell` and `make check-functional-test-ids` bare. Expect exit 0 from both.

3. Run `make functional-test` bare, in the background, with no `NO_COLOR` in the environment.
   Expect a green run whose summary counts the new test id as a pass.

4. Controlled fault: change `.with_ansi(stderr_ansi)` to `.with_ansi(true)`, rebuild, re-run
   `make functional-test`. Expect `verbose-tracing-is-plain-on-redirected-stderr` to report FAIL
   with `stderr unexpectedly contains`. Revert and confirm it passes again.

**Acceptance.** The test passes with the fix and fails without it, demonstrated in that order.

## Task 3 — the comparison tier runs against a container it created

**Interfaces.** Consumes `tests/functional/setup-bugzilla.sh`'s existing `reset` command,
dispatched from its `case` block to `cmd_reset`, which calls `cmd_stop` then `cmd_start`;
`cmd_stop` uses `rm -f … 2>/dev/null || true` and so succeeds on an absent container. Defines
nothing.

**Verification.**

- Contract: `make functional-compare` and `run-compare-all.sh` recreate the container instead of
  reusing a running one. `Mode: task-test-not-applicable` — the changed surface is two
  build-recipe lines whose only observable effect is container lifecycle on a Docker/podman host.
  No test in this repository drives a container runtime or observes make-recipe prerequisites.
  Proof is step 4's manual sequence, which is the precondition the change exists to enforce.

**Steps.**

1. In `Makefile`, replace the `functional-compare` target — currently
   `functional-compare: release functional-start ## Compare bzr and python-bugzilla` over a
   single `BZR_COMPARE_BIN="$(BZR_COMPARE_BIN)" tests/functional/run-compare.sh` recipe line —
   with:

   ```make
   # The comparison tier asserts exact result sets -- the server-side saved-search
   # precondition matches exactly two bug ids -- so it needs a container whose corpus
   # an earlier `make functional-test` run has not grown. `reset` recreates it; the
   # `functional-start` prerequisite this target used to carry would reuse a dirty one.
   functional-compare: release ## Compare bzr and python-bugzilla (recreates the container)
   	tests/functional/setup-bugzilla.sh reset
   	BZR_COMPARE_BIN="$(BZR_COMPARE_BIN)" tests/functional/run-compare.sh
   ```

2. In `tests/functional/run-compare-all.sh`, change
   `if ! "$SCRIPT_DIR/setup-bugzilla.sh" start; then` to
   `if ! "$SCRIPT_DIR/setup-bugzilla.sh" reset; then`. Leave the
   `RESULTS+=("${ver}: FAILED (container start)")` message unchanged: a `reset` that fails, fails
   inside `cmd_start`.

3. Run `make check-shell` bare. Expect exit 0.

4. Run, in this order with no `NO_COLOR` in the environment: `make functional-test`, then
   `make functional-compare`. Expect the comparison run to reach its own summary rather than
   failing on the saved-search precondition, and to report a transport per capability rather than
   `transport observation is missing`.

**Acceptance.** The comparison tier passes from a checkout that has just run the functional tier,
with no environment variable set by the caller.

## Rollback

Every change is additive or a short substitution; `git revert` of the branch restores the
previous behaviour with no data, schema, or config migration. A host left holding a stopped
container after a failed `reset` is repaired by `make functional-start`, as before.

## Deferrals

None recorded.
