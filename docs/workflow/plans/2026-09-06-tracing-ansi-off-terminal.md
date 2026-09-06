# Plan: plain-text tracing off a terminal, and a fresh comparison container

Goal: bzr must not write ANSI escapes to a stderr that is not a terminal, so the comparison
harness observes transport with no caller-supplied environment variable, and the comparison tier
must run against a container created for that run.

Architecture: `src/main.rs` gains one pure helper deciding the tracing stream's ANSI setting;
`src/main_tests.rs` covers it. `tests/functional/phases/17-global-options.sh` gains one test.
`Makefile` and `tests/functional/run-compare-all.sh` swap `setup-bugzilla.sh start` for the
existing `reset`. `docs/bzr-cli.md:50` and the clap doc comment at `src/cli/mod.rs:247-251` are
corrected together, being the two renderings of one flag's contract. Spec:
`docs/workflow/specs/2026-09-06-tracing-ansi-off-terminal-design.md`. ADR:
`docs/adr/0058-suppress-tracing-ansi-off-terminal.md`. Branch
`feat/suppress-tracing-ansi-on-non-tty-722` off `main`.

Expected implementation size: 85-100 changed lines (S) - `src/main.rs` ~24, `src/main_tests.rs`
~31, phase 17 = 9, the two doc surfaces = 11, `Makefile` = 10, `run-compare-all.sh` = 2.

## Global Constraints

Repository conventions in `CLAUDE.md` bind every task; these are the ones it does not state.

- MSRV `1.89` (`Cargo.toml:14`), so `Option::is_none_or` (1.82) and `OsStr::is_empty` (1.9) are
  both available. `tracing-subscriber` is pinned `=0.3.23`.
- `src/main_tests.rs` already exists, starts with `#![expect(clippy::expect_used)]`, then
  `use super::*;`.
- `tests/functional/phases/*.sh` are shellcheck'd (`make check-shell`) but not shfmt'd, and use
  4-space indent. Test ids match `^[a-z0-9]+(-[a-z0-9]+)*$` and must be unique
  (`make check-functional-test-ids`).
- Concurrent issues own `src/client/auth/`, `src/client/transport.rs`,
  `tests/functional/phases/08c-bugs-create-fields.sh`, `tests/functional/pybz/container-tests.sh`,
  and `tests/functional/redhat-shape-proxy.py`. Do not edit them.

## Task 1 — bzr writes plain text to a non-terminal stderr

**Interfaces.** Defines `tracing_ansi_enabled` (step 3) in `src/main.rs`, private to that binary
crate and reached by `src/main_tests.rs` through its existing `use super::*;`. Consumes
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
   `no_color_env.is_none_or(std::ffi::OsStr::is_empty)`. Doc comment: `tracing_subscriber::fmt`
   defaults ANSI on unless `NO_COLOR` holds a non-empty value and performs no terminal detection,
   and an explicit `with_ansi` call replaces that default outright, so the helper must re-apply
   the `NO_COLOR` rule rather than inherit it.
4. In `main`, insert `let stderr_ansi = tracing_ansi_enabled(cli.no_color,
   std::env::var_os("NO_COLOR").as_deref(), std::io::stderr().is_terminal());` above the
   `tracing_subscriber::fmt()` chain at `src/main.rs:29-32`, and add `.with_ansi(stderr_ansi)` to
   that chain between `.with_env_filter(filter)` and `.with_writer(std::io::stderr)`.
5. Run `make test-one T=tracing_ansi`. Expect all six cases to pass.
6. Correct both renderings of the `--no-color` contract, each staying its current size:
   `docs/bzr-cli.md:50` (currently `Disable colored output. Color is also suppressed
   automatically when stdout is not a TTY.`) says the flag covers stdout and the stderr
   diagnostic stream and that each stream's automatic suppression follows its own terminal
   status; the clap doc comment at `src/cli/mod.rs:247-251` (currently claiming
   `CLICOLOR_FORCE=1` re-enables colour unqualified) says `NO_COLOR`, `CLICOLOR=0`, and
   `CLICOLOR_FORCE=1` govern stdout colour and that the flag disables colour on both streams.
7. Run `make lint` bare, then `make test` bare in the background. Expect exit 0 from both.

**Acceptance.** For a debug build,
`BZKEY=x ./target/debug/bzr -vv --server-url http://127.0.0.1:9 --server-api-key-env BZKEY bug view 1 2>capture`
leaves `grep -c $'\033' capture` printing `0`, while the same command with stderr on a terminal
still shows colour. `bzr --help` shows the corrected `--no-color` text.

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
   - **Transport half:** with the correct build, append a character that cannot occur to both
     `BZR_REST_BOUNDARY_RE` and `BZR_XMLRPC_BOUNDARY_RE` (`tests/functional/lib.sh:272-275`),
     re-run `make functional-test`. Expect FAIL with
     `transport is not observable from -vv stderr`. Revert; confirm it passes again.

**Acceptance.** The test passes with the fix and fails under each fault for its own distinct
reason, demonstrated in that order.

## Task 3 — the comparison tier runs against a container it created

**Interfaces.** Consumes `setup-bugzilla.sh`'s existing `reset` command (`case` dispatch at
`:208` → `cmd_reset` at `:186-190` = `cmd_stop` then `cmd_start`; `cmd_stop` at `:153` uses
`rm -f … 2>/dev/null || true` and so succeeds on an absent container). Defines nothing.

**Verification.**

- Contract: both entry points recreate the container instead of reusing a running one.
  `Mode: task-test-not-applicable` — the changed surface is two build-recipe lines whose only
  observable effect is container lifecycle on a Docker/podman host. No test in this repository
  drives a container runtime or observes make-recipe prerequisites. Proof is step 4.

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
   `RESULTS+=("${ver}: FAILED (container start)")` message at `:30` unchanged. `cmd_reset` ends
   in `return 0`, but that line is unreachable on failure: `setup-bugzilla.sh:5` sets
   `set -euo pipefail`, `cmd_start` calls `resolve_bz_port || exit 1`, and its final
   `wait_for_ready` returns 1 on a container that never becomes ready, so a failing `reset` still
   exits non-zero.
3. Run `make check-shell` bare. Expect exit 0.
4. Run, in this order with no `NO_COLOR` in the environment: `make functional-test`, then
   `make functional-compare`. Expect the comparison run to reach its own summary and to report a
   transport per capability rather than `transport observation is missing`.

**Acceptance.** The comparison tier passes from a checkout that has just run the functional tier,
with no environment variable set by the caller.

## Rollback

Every change is additive or a short substitution; `git revert` of the branch restores the previous
behaviour with no data, schema, or config migration. A host left holding a stopped container after
a failed `reset` is repaired by `make functional-start`, as before.

## Deferrals

None recorded.
