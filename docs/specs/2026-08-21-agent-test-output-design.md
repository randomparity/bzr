# Agent-friendly unit-test output — design

Issue: [#539](https://github.com/randomparity/bzr/issues/539)
Decision record: [ADR 0019](../adr/0019-quiet-by-default-unit-test-output.md)

## Goal

Make the default unit-test invocation usable inside an agent loop: minimal
output by default, full output on explicit request, and documented guidance so
agents (and humans) select the right mode.

## Requirements

- **R1 — Quiet default.** `make test` suppresses per-test success lines
  (`test <name> ... ok`) and cargo compilation status lines (`Compiling`,
  `Finished`). rustc warnings and errors still print, as do libtest's
  per-suite `running N tests` header and its compact quiet progress markers.
  Failing tests must still print their captured stdout/stderr, the failure
  list, and the summary.
- **R2 — Verbose mode.** An explicit opt-in restores the current full
  `cargo test` output, available both as `VERBOSE=1 make test` and as a
  discoverable `make test-verbose` target.
- **R3 — Documented selection.** AGENTS.md documents both modes and states
  when each is appropriate: the quiet default for routine verification;
  `make test-verbose` when diagnosing a failure that needs the full per-test
  listing or compilation detail (quiet mode already prints failing tests'
  captured output and summaries). CLAUDE.md is covered by the existing
  symlink to AGENTS.md.
- **R4 — Guardrails stay green.** `make lint` and `make test` pass; CI
  workflows, git hooks, functional-test targets, and all production Rust code
  are unchanged.

## Design

One mechanism, in the Makefile only:

```make
# Agent ergonomics: `make test` runs quiet by default so agent loops keep
# their context small; VERBOSE=1 (or `make test-verbose`) restores the full
# cargo output for debugging.
test: ## Run tests (quiet by default; VERBOSE=1 or test-verbose for full output)
	$(CARGO) test $(if $(filter 1,$(VERBOSE)),,--quiet)

test-verbose: ## Run tests with full output (same as VERBOSE=1 make test)
	$(MAKE) --no-print-directory VERBOSE=1 test
```

Properties:

- Only the `test` target consults `VERBOSE`; every other target is untouched.
- Verbosity is controlled solely by `VERBOSE=1`. There is deliberately no
  `TEST_FLAGS` override surface: a named variable would let a command-line or
  exported value silently outrank the `VERBOSE` switch, and the contract this
  design exists to keep is that `VERBOSE=1 make test` is always verbose.
- Exactly `VERBOSE=1` enables verbose output; any other value (including
  `VERBOSE=true`) leaves quiet mode on. This strictness is deliberate and
  documented, because "truthy" handling differs across tools and a silent
  mismatch is hard to diagnose.

## Failure modes considered

- *Quiet hides an error an agent needs.* Rejected as unreachable: libtest
  always prints failing-test captured output and failure summaries regardless
  of `-q`. ADR 0019 records an independent scratch-crate verification of this
  property (2026-08-21); implementation must re-verify it against this
  repository's real suites per plan Task 1 step 8 before the change lands.
- *Agent sets VERBOSE=true expecting verbose.* Output stays quiet; AGENTS.md
  documents that only `1` counts, and `make test-verbose` exists as the
  unambiguous spelling.

## Testing

The Makefile has no automated test harness in this repository; verification is
behavioral, executed during implementation and recorded in the plan's steps:

1. `make test` completes and prints a summary without per-test `ok` lines.
2. `make test-verbose` and `VERBOSE=1 make test` print per-test lines.
3. A temporarily introduced failing test still prints its assertion message
   under `make test` (then removed).
4. `make lint` passes.

## Out of scope

CI workflow files, git hooks, functional tests, production Rust code, and any
new tooling such as cargo-nextest (see ADR 0019).
