# Plan: plain-text tracing off a terminal, and a fresh comparison container

Goal: bzr must not write ANSI escapes to a stderr that is not a terminal, so the comparison
harness observes transport with no caller-supplied environment variable, and the comparison tier
must run against a container created for that run.

Architecture: `src/main.rs` gains one pure helper deciding the tracing stream's ANSI setting;
`src/main_tests.rs` covers it. `tests/functional/phases/17-global-options.sh` gains one test.
`Makefile` and `tests/functional/run-compare-all.sh` swap `setup-bugzilla.sh start` for the
existing `reset`, and `setup-bugzilla.sh` `cmd_reset` gains a removal check. Five
colour-documentation surfaces are corrected together — the `--no-color`, `NO_COLOR`, `CLICOLOR`,
and `CLICOLOR_FORCE` rows of `docs/bzr-cli.md`, plus the clap doc comment for `--no-color` in
`src/cli/mod.rs`. Spec:
`docs/workflow/specs/2026-09-06-tracing-ansi-off-terminal-design.md`. ADR:
`docs/adr/0058-suppress-tracing-ansi-off-terminal.md` — numbered 0058 because 0054-0057 are
assigned to concurrent issues in the same campaign wave, not because records are missing. Its
`docs/adr/README.md` row is the orchestrator's to append after the wave merges; CI gates no ADR
index, so no task adds it. Branch `feat/suppress-tracing-ansi-on-non-tty-722` off `main`.

Expected implementation size: 100-120 changed lines (S) - `src/main.rs` ~24, `src/main_tests.rs`
~31, phase 17 = 9, the five colour-documentation surfaces = 19, `setup-bugzilla.sh` = 7,
`Makefile` = 10, `run-compare-all.sh` = 2.

## Global Constraints

Repository conventions in `CLAUDE.md` bind every task; these are the ones it does not state.

- MSRV `1.89` (`Cargo.toml:14`), so `Option::is_none_or` (1.82) and `OsStr::is_empty` (1.9) are
  both available. `tracing-subscriber` is pinned `=0.3.23`.
- `src/main_tests.rs` already exists, starts with `#![expect(clippy::expect_used)]`, then
  `use super::*;`.
- `tests/functional/phases/*.sh` are shellcheck'd (`make check-shell`) but not shfmt'd, and use
  4-space indent. Test ids match `^[a-z0-9]+(-[a-z0-9]+)*$` and must be unique
  (`make check-functional-test-ids`).
- Concurrent issues own the files the charter excludes; do not edit them.

## Task 1 — bzr writes plain text to a non-terminal stderr

**Interfaces.** Defines, in `src/main.rs`, private to that binary crate and reached by
`src/main_tests.rs` through its existing `use super::*;`:
`fn tracing_ansi_enabled(no_color_flag: bool, no_color_env: Option<&std::ffi::OsStr>,
stderr_is_terminal: bool) -> bool`. Consumes
`Cli::no_color` (`src/cli/mod.rs:253`, `pub no_color: bool`), `std::io::IsTerminal` (imported at
`src/main.rs:1`), and `tracing_subscriber::fmt::SubscriberBuilder::with_ansi` (0.3.23,
`src/fmt/mod.rs:633`, `pub fn with_ansi(self, ansi: bool) -> SubscriberBuilder<…>`).

**Verification.**

- Contract: the truth table below. `Mode: focused-test` — `src/main_tests.rs`, one `#[test]` per
  row, named `tracing_ansi_*`. Expected red: they do not compile,
  ``cannot find function `tracing_ansi_enabled` ``. Green: `make test-one T=tracing_ansi`.
- Contract: `docs/bzr-cli.md:50` and the clap doc comment describe the behaviour bzr now has.
  `Mode: task-test-not-applicable` — prose in a hand-written reference table and a clap doc
  comment, with no executable consumer; a test grepping for the sentence would assert wording,
  not behaviour. The behaviour they describe is covered by the cases above.

| `no_color_flag` | `no_color_env` | `stderr_is_terminal` | result |
|---|---|---|---|
| `false` | `None` | `true` | `true` |
| `false` | `Some("")` | `true` | `true` |
| `false` | `None` | `false` | `false` |
| `true` | `None` | `true` | `false` |
| `false` | `Some("1")` | `true` | `false` |
| `false` | `Some("")` | `false` | `false` |

**Steps.**

1. Add one `#[test]` per truth-table row to `src/main_tests.rs`, each a single
   `assert!(tracing_ansi_enabled(<flag>, <env>, <tty>));` or its negation, with
   `use std::ffi::OsStr;` added to the file's imports and `OsStr::new("…")` supplying the `Some`
   values.
2. Run `make test-one T=tracing_ansi`. Expect a compile error naming `tracing_ansi_enabled`.
3. Add the helper to `src/main.rs`, below `tracing_filter_directive`, with the signature above.
   Body: return `false` when `no_color_flag` is set or `stderr_is_terminal` is false, then
   `no_color_env.is_none_or(std::ffi::OsStr::is_empty)`. Give it the doc comment ADR 0058's
   Decision records.
4. In `main`, insert `let stderr_ansi = tracing_ansi_enabled(cli.no_color,
   std::env::var_os("NO_COLOR").as_deref(), std::io::stderr().is_terminal());` above the
   `tracing_subscriber::fmt()` chain at `src/main.rs:29-32`, and add `.with_ansi(stderr_ansi)` to
   that chain between `.with_env_filter(filter)` and `.with_writer(std::io::stderr)`.
5. Run `make test-one T=tracing_ansi`. Expect all six cases to pass.
6. Correct five colour-documentation surfaces, each staying its current size. Locate the four
   `docs/bzr-cli.md` rows by their leading cell rather than by line number — run
   `grep -n 'NO_COLOR\|CLICOLOR\|`--no-color`' docs/bzr-cli.md` first, because the numbers move.
   - the `--no-color` row, and the clap doc comment for the same flag in `src/cli/mod.rs`: the
     flag disables colour on stdout and on the stderr diagnostic stream, and each stream's
     automatic suppression follows its own terminal status.
   - the `NO_COLOR` row: state it per stream, because the two disagree on an empty value. Any
     present `NO_COLOR` disables stdout colour — `colored` 3.1.1 `src/control.rs:144-158` maps
     `Ok(s)` to `Some(s != "0")`, so `NO_COLOR=` and `NO_COLOR=0` both count as present — while a
     **non-empty** `NO_COLOR` suppresses tracing ANSI, following the crate default and
     no-color.org. Keep the row's existing "any value" claim for stdout; it is correct. Do not
     write that one rule covers both streams.
   - the `CLICOLOR` and `CLICOLOR_FORCE` rows, and the same claim in the clap comment: both govern
     stdout colour only, and `CLICOLOR_FORCE=1` does not force colour when stdout is redirected,
     because `src/main.rs:36-37` sets `colored`'s manual override and `colored` 3.1.1's
     `ShouldColorize::should_colorize` (`src/control.rs:118-128`) reads that override first.
7. Run `make lint` bare, then `make test` bare in the background. Expect exit 0 from both.

**Acceptance.** `grep -c $'\033'` over a redirected `bzr -vv` stderr prints `0` on a debug
build, colour survives on a terminal, and `bzr --help` shows the corrected `--no-color` text.

## Task 2 — the functional tier proves it and bites on both halves

**Interfaces.** Consumes `run_bzr_raw`, `assert_success`, `assert_stderr_not_contains`,
`test_begin` / `test_pass` / `test_fail` / `test_skip`, and `observe_bzr_transport`, all in
`tests/functional/lib.sh` and in scope for a sourced phase, plus `$BUG1`, set by `08-bugs.sh:14`
and guarded the same way by tests already in this file. Defines nothing.

**Verification.**

- Contract: a real `bzr -vv` invocation whose stderr is a file yields a stderr with no escape
  byte, from which `observe_bzr_transport` derives a transport. `Mode: focused-test` —
  `tests/functional/phases/17-global-options.sh`, test id
  `verbose-tracing-is-plain-on-redirected-stderr`. Expected red: the two distinct faults in
  step 4. Green: `make functional-test` with Task 1 in place.

**Steps.**

1. In `tests/functional/phases/17-global-options.sh`, immediately after the
   `verbose-response-body-diagnostics-redact-api-keys` test and its `unset` line (`:51-62`), add
   at 4-space indent:

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
4. Two controlled faults, applied and reverted one at a time. The `&&` short-circuits, so the
   first fault never reaches `observe_bzr_transport` and cannot prove the transport half.
   - **ANSI half:** change `.with_ansi(stderr_ansi)` to `.with_ansi(true)`, rebuild, re-run
     `make functional-test`. Expect FAIL with `stderr unexpectedly contains`. Revert; confirm it
     passes again.
   - **Transport half:** with the correct build, break the emitted boundary line itself, which is
     what criterion 4 names. Temporarily change the REST message `"API response"`
     (`src/client/transport.rs:127`) to something the harness regex cannot match, rebuild, and
     re-run `make functional-test`. Expect FAIL with
     `transport is not observable from -vv stderr` — a different message from the ANSI fault's,
     which is the point. `src/client/transport.rs` is a charter exclusion owned by issue #715, so
     this edit is a local fault that never reaches a commit: revert it, confirm
     `git status --short -- src/client/transport.rs` is empty, and confirm the test passes again.
     If touching that file is unwelcome, the equivalent harness-side fault is to append an
     impossible character to `BZR_REST_BOUNDARY_RE` and `BZR_XMLRPC_BOUNDARY_RE`
     (`tests/functional/lib.sh:272-275`); it drives the same failure path but proves the
     observation rather than the emitter, so record the substitution if it is used.

**Acceptance.** The test passes with the fix and fails under each fault for its own reason.

## Task 3 — the comparison tier runs against a container it created

**Interfaces.** Consumes `setup-bugzilla.sh`'s existing `reset` command (`case` dispatch at
`:208` → `cmd_reset` at `:186-190` = `cmd_stop` then `cmd_start`), and its existing
`container_exists` and `err` helpers. Modifies `cmd_reset`; leaves `cmd_stop` alone, since its
`rm -f … 2>/dev/null || true` at `:153` is what lets `stop` succeed on an absent container for
every other caller.

**Verification.**

- Contract: both entry points recreate the container instead of reusing a running one, and
  `reset` fails loudly rather than silently reusing when removal is refused.
  `Mode: task-test-not-applicable` — the changed surface is build-recipe lines and one container
  lifecycle branch on a Docker/podman host. No test in this repository drives a container runtime
  or observes make-recipe prerequisites. Proof is steps 4 and 5.

**Steps.**

1. In `Makefile`, replace the `functional-compare` target at `:200-201` — currently
   `functional-compare: release functional-start ## Compare bzr and python-bugzilla` over one
   `BZR_COMPARE_BIN="$(BZR_COMPARE_BIN)" tests/functional/run-compare.sh` recipe line. Drop the
   `functional-start` prerequisite, retitle the help text
   `## Compare bzr and python-bugzilla (recreates the container)`, and make the recipe run
   `tests/functional/setup-bugzilla.sh reset` before the unchanged `run-compare.sh` line, so the
   ordering is guaranteed by the recipe rather than by prerequisite evaluation. Precede the
   target with a comment saying that the tier is meant to compare against a server the run set
   up, that `functional-start` silently reuses a container an earlier `make functional-test` left
   running, and that the precise coupling is uncharacterised (see ADR 0058).
2. In `tests/functional/run-compare-all.sh:29`, change
   `if ! "$SCRIPT_DIR/setup-bugzilla.sh" start; then` to
   `if ! "$SCRIPT_DIR/setup-bugzilla.sh" reset; then`, leaving the
   `RESULTS+=("${ver}: FAILED (container start)")` message at `:30` unchanged: `setup-bugzilla.sh:5`
   sets `set -euo pipefail`, so a failing `reset` exits non-zero and that branch still reports it.
3. In `tests/functional/setup-bugzilla.sh`, make `cmd_reset` (`:186-190`) verify removal before
   restarting: after `cmd_stop`, if `container_exists` still succeeds, call `err` with a message
   naming the likely cause — a dependent container such as a leftover python-bugzilla sidecar,
   which `tests/functional/lib.sh:406` attaches with `--network container:<name>` and which
   podman refuses to orphan — and the repair, which is to remove that sidecar
   (`pybz_sidecar_stop`, `tests/functional/lib.sh:430`, or `<runtime> rm -f <sidecar>`) and retry;
   then `return 1`. `cmd_stop` discards `rm -f` failure and logs "Container removed." regardless,
   so without this check a refused removal makes `reset` silently identical to `start`.
4. Run `make check-shell` bare. Expect exit 0.
5. Run, in this order with no `NO_COLOR` in the environment: `make functional-test`, then
   `make functional-compare`. Expect the comparison run to reach its own summary and to report a
   transport per capability rather than `transport observation is missing`.

**Acceptance.** The comparison tier passes from a checkout that has just run the functional
tier, with no environment variable set by the caller.

## Rollback

`git revert` of the branch restores the previous behaviour; no data, schema, or config migration
is involved. A `reset` refused because a dependent sidecar holds the container leaves that
container **running**, so `make functional-start` is inert there and reports success: the repair
is to remove the sidecar and re-run. A host left holding a *stopped* container is still repaired
by `make functional-start`, as before.

## Deferrals

None recorded.
