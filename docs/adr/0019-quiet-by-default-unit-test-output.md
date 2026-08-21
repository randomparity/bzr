# ADR 0019: Quiet-by-default unit-test output with an explicit verbose mode

## Status

Accepted

## Context

Issue #539 asks for a better agent experience when running unit tests. The
default `cargo test` (and therefore `make test`) prints one line per test plus
full compilation progress. An agent running tests inside its loop spends
context on per-test `ok` lines and compile chatter while the information it
actually needs — pass/fail summary and failure details — is a small fraction of
the output. The repository's agent guidance (AGENTS.md, mirrored by the
CLAUDE.md symlink) currently offers no quieter alternative and no guidance on
when full output is warranted.

## Decision
`make test` runs `cargo test --quiet` by default. A `VERBOSE=1` variable
restores the full, unquiet cargo output — only the exact value `1` enables
verbose mode; any other value keeps quiet mode — and a dedicated
`make test-verbose` target delegates to `VERBOSE=1 make test` so the mode is
discoverable as a target, not only as a flag. AGENTS.md documents both and
states the selection rule: use the quiet default for routine verification;
switch to `make test-verbose` when diagnosing a failure that needs the full
per-test listing or captured output that quiet mode does not surface.

`cargo test --quiet` keeps failure behavior intact: failing tests still print
their captured stdout/stderr, the failure list, and the summary. Quiet mode
removes per-test success lines and compilation noise only. CI, the pre-commit
hook, the pre-push hook, and the functional-test targets are unchanged; they
run unattended where full output costs nothing.

## Consequences

- Agent test loops read a few summary lines instead of hundreds of per-test
  lines, without losing failure details.
- `make test` output shape changes for everyone, not only agents; developers
  who prefer the per-test listing use `make test-verbose`.
- The verbosity contract lives in the Makefile, so `cargo test` invocations
  made directly (hooks, CI, docs) keep their existing behavior and output.
- `VERBOSE` is a conventional, generic variable name; only the `test` target
  consults it, and any future target that adopts it must document that here.

## Considered & rejected
- **Adopt cargo-nextest as the test runner.** judgment: a new required tool,
  provisioned on every development and CI build host, is justified by missing
  capability, and output shaping is not a missing capability — the standard
  libtest already provides the failure behavior this change needs.
- **Keep the status quo (always full output).** judgment: the cost lands on
  every agent test run, and issue #539 exists precisely because that cost is
  not acceptable.
- **Wrap cargo in a `tools/` script that filters output.** judgment: a second
  indirection layer that duplicates what a make variable expresses in two
  lines, and a new place output behavior can drift from plain `cargo test`.
- **Quiet the pre-push hook and CI too.** verified: `.git/hooks/pre-push` runs
  bare `cargo test` and `.github/workflows/ci.yml` runs `cargo test --locked
  --features test-helpers`; both run unattended, where verbose output is free,
  and changing installed hooks would silently diverge from already-installed
  copies until `make install-hooks` re-runs.
