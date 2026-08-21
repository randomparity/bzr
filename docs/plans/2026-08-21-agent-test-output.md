# Plan: Agent-friendly unit-test output

**Goal:** `make test` runs quiet by default; `VERBOSE=1 make test` / `make
test-verbose` restore full output; AGENTS.md documents the selection rule.

**Architecture:** A Makefile-only change. The `test` target's cargo invocation
gains a `TEST_FLAGS` variable that defaults to `--quiet` and is emptied when
`VERBOSE` is exactly `1`. A new `test-verbose` phony target delegates to
`$(MAKE) --no-print-directory VERBOSE=1 test`. Documentation changes live in
AGENTS.md (CLAUDE.md is its symlink).

**Tech stack:** GNU make, cargo. No new dependencies.

**Global Constraints** (from the spec, transcribed):

- Failing tests must still print their captured stdout/stderr, the failure
  list, and the summary under quiet mode (spec R1).
- Exactly `VERBOSE=1` enables verbose output; any other value, including
  `VERBOSE=true`, stays quiet (spec, strictness paragraph).
- Only the `test` target consults `VERBOSE`; no other target changes behavior
  (spec R4, design properties).
- CI workflows, git hooks, functional-test targets, and production Rust code
  are unchanged (spec R4).
- Guardrails: `make lint` must pass before each commit (repository AGENTS.md).
- Repo conventions: conventional-commit subject ≤72 chars, imperative mood;
  stage explicit paths only.
- Decision record: ADR 0019 (`docs/adr/0019-quiet-by-default-unit-test-output.md`).

## Files

- `Makefile` — modified (test target region, `.PHONY` line, new `test-verbose`
  target). Owns the verbosity contract.
- `AGENTS.md` — modified (Build & Development Commands block + a short
  selection-guidance paragraph). CLAUDE.md follows via symlink.

## Task 1: Makefile verbosity control

**Files:** modifies `Makefile`; verifies behavior by running `make`.

**Interfaces:** consumes nothing; later tasks and all developers rely on:
`make test` = quiet run, `make test-verbose` = full run, `VERBOSE=1 make test`
= full run, any other `VERBOSE` value = quiet run.

**Steps:**

1. Baseline the current output shape (for the comparison in the acceptance
   criteria): run `make test 2>&1 | tail -5` and note that per-test `running N
   tests` / `test <name> ... ok` lines appear. (No edit yet.)
2. In `Makefile`, extend the `.PHONY` line to include `test-verbose`. The
   current line 4-11 block lists phony targets; add `test-verbose` to it.
3. Insert the verbosity control immediately above the `test:` target (after
   the `release:` target), verbatim:

   ```make
   # Agent ergonomics: `make test` runs quiet by default so agent loops keep
   # their context small; VERBOSE=1 (or `make test-verbose`) restores the full
   # cargo output for debugging.
   TEST_FLAGS ?= --quiet
   ifeq ($(VERBOSE),1)
   TEST_FLAGS :=
   endif
   ```

4. Change the `test` target body from `$(CARGO) test` to
   `$(CARGO) test $(TEST_FLAGS)` and its help comment to
   `## Run tests (quiet by default; VERBOSE=1 or test-verbose for full output)`.
5. Add the new target immediately after `test`:

   ```make
   test-verbose: ## Run tests with full output (same as VERBOSE=1 make test)
   	$(MAKE) --no-print-directory VERBOSE=1 test
   ```

   (Recipe lines are indented with a literal TAB.)
6. Verify quiet default: run `make test 2>&1 | tail -5`. Expected: a summary
   line of the form `test result: <n> passed; 0 failed; ...` (or cargo's
   quiet equivalent) and **no** `test <name> ... ok` lines in the tail.
7. Verify verbose target: run `make test-verbose 2>&1 | grep -c '^test .* \.\.\. ok'`
   or, if the count is large, `make test-verbose 2>&1 | grep -m3 'test .* \.\.\. ok'`.
   Expected: at least one `test <name> ... ok` line (the full per-test listing
   is back). Also run `VERBOSE=1 make test 2>&1 | grep -m1 'test .* \.\.\. ok'`
   — expected: one such line.
8. Verify failure visibility: temporarily add to the end of
   `src/error_tests.rs` (an existing sibling test file):

   ```rust
   #[test]
   fn tmp_q539_quiet_failure_visibility() {
       assert_eq!(1, 2, "q539 temporary failure-visibility probe");
   }
   ```

   Run `make test 2>&1 | grep -c 'q539 temporary failure-visibility probe'`.
   Expected: count ≥ 1 (the assertion message prints under quiet mode). Then
   remove the temporary test and confirm `git diff` shows it gone.
9. Verify strictness: run `VERBOSE=true make test 2>&1 | grep -m1 'test .* \.\.\. ok' || echo quiet`
   — expected output: `quiet`.
10. Run `make lint`. Expected: exit 0, no warnings.

**Acceptance criteria:**

- `make test` prints no per-test success lines; failures still print details
  (step 8 proof).
- `make test-verbose` and `VERBOSE=1 make test` print per-test success lines.
- `VERBOSE=true make test` stays quiet.
- `make lint` exits 0.
- `git diff Makefile` shows only the `.PHONY` addition, the flag block, the
  test-target change, and the new target.

**Commit:** `git add Makefile && git commit -m "build: quiet make test by default with VERBOSE escape hatch"`

## Task 2: AGENTS.md guidance

**Files:** modifies `AGENTS.md` (CLAUDE.md follows via symlink; verify with
`readlink CLAUDE.md` → `AGENTS.md`).

**Interfaces:** consumes the Task 1 contract (`make test`, `make test-verbose`,
`VERBOSE=1`).

**Steps:**

1. In the `Build & Development Commands` code block, after the
   `cargo test <test_name>` line, add:

   ```
   make test                         # Run tests (quiet output; failures still print details)
   make test-verbose                 # Run tests with full cargo output (or: VERBOSE=1 make test)
   ```

2. Add a short paragraph directly below the `make setup` / git-hooks
   paragraph:

   ```markdown
   Test output selection for agent loops: `make test` runs quiet by default —
   a summary line per suite, with failing tests still printing their captured
   output and a full failure summary — which is the right default for routine
   verification. Use `make test-verbose` (or `VERBOSE=1 make test`) only when
   diagnosing a failure that needs the full per-test listing or compilation
   detail; only the exact value `VERBOSE=1` enables it.
   ```

3. Verify the symlink covers CLAUDE.md: `readlink CLAUDE.md` — expected
   `AGENTS.md`; no second edit needed.
4. Run `make lint` (docs-only change, but the hook runs it anyway). Expected:
   exit 0.

**Acceptance criteria:**

- `grep -c 'test-verbose' AGENTS.md` ≥ 2 (command block + guidance paragraph).
- `readlink CLAUDE.md` = `AGENTS.md`.
- `make lint` exits 0.

**Commit:** `git add AGENTS.md && git commit -m "docs: document quiet test default and verbose selection"`
