# Python-bugzilla comparison harness implementation plan

**Goal:** Run bzr and python-bugzilla 3.3.0 against the same real Bugzilla instances and report
semantic comparison outcomes.

**Architecture:** A pinned long-lived python-bugzilla sidecar joins each functional Bugzilla
container's network namespace. A dedicated runner reuses the functional test library, sources a
separate comparison phase tree, and reports pass/fail/skip/expected-gap counts.

**Tech stack:** Bash scripts, Docker/Podman CLI, Make, GitHub Actions, official
`python:3.14.7-slim-bookworm` container with `python-bugzilla==3.3.0`, jq, rg, shellcheck.

**Expected implementation size: 420–700 changed lines (L) — derived from four shell/runtime
contracts, guard fixtures, Make/CI wiring, one phase, and one report.**

## Global constraints

- Support Bugzilla versions bz50, bz52, and bz53 in the existing Ubuntu scheduled workflow and on
  local environments providing Bash, Make, jq, rg, and Docker/Podman.
- Preserve the existing functional scripts' Bash and BSD/GNU userland portability; use existing
  Docker/Podman detection and checkout-scoped identity helpers.
- Pin python-bugzilla exactly to 3.3.0; add no host Python dependency and no Rust dependency.
- Never mount the repository root, host home, credentials, or runtime socket into the sidecar.
- Keep ADR 0029 semantic IDs and ADR 0030 checkout/version isolation intact.
- Guardrails: `make lint`, `make test`, `make functional-compare-all`, and
  `make functional-test-all`.
- Branch: `feat/python-bugzilla-comparison-harness-666`; base: `main`.

## Task 1: Parameterize the semantic functional-test ID guard

**Files:** modify `tools/check-functional-test-ids.sh`,
`tools/check-functional-test-ids-tests.sh`, and `Makefile`.

**Interfaces:** The checker consumes `<repo-root> [runner-relative-path phase-dir-relative-path]`.
The Makefile invokes it once with defaults and once with `tests/functional/run-compare.sh` plus
`tests/functional/compare`. It derives the `compare` namespace from the custom phase directory;
`test_begin` consumes `TEST_ID_PREFIX` and emits
`${TEST_ID_PREFIX:+$TEST_ID_PREFIX/}<phase>/<slug>`. Task 3 supplies that runner and directory.

**Verification**

- Mode: focused-test — alternate runner/phase parameters, source-directory validation, exact
  `compare/<phase>/<slug>` runtime/static identity, and cross-tree phase/slug distinction;
  `tools/check-functional-test-ids-tests.sh`; red result is rejection of a valid alternate tree or
  acceptance of a mismatched source; green command is
  `bash tools/check-functional-test-ids-tests.sh`, expected exit 0 and its pass summaries.

**Steps**

1. Extend fixture helpers to create an alternate runner and phase tree, then assert valid custom
   paths pass and a mismatched source directory fails.
2. Run `bash tools/check-functional-test-ids-tests.sh`; expect the new valid fixture to fail because
   the checker treats the second argument as unsupported or ignores it.
3. Parse optional runner and phase-directory paths relative to the repository root, derive the
   expected source directory basename/namespace, and make the canonical-loop parser require the
   matching source path and runtime prefix.
4. Run the fixture script; expect exit 0 with both runtime and ID checker pass lines.
5. Keep the Make target on its existing default invocation until Task 3 creates the comparison
   runner and phase tree, then commit the checker and fixture changes.

**Acceptance:** Existing default calls behave unchanged; custom runner/tree validation is strict and
covered by fixtures.

## Task 2: Add comparison result and sidecar primitives

**Files:** modify `tests/functional/lib.sh`; create
`tests/functional/pybz/Containerfile` and `tests/functional/pybz/container-tests.sh`.

**Interfaces:** Reuse `BZR_STDOUT`, `BZR_STDOUT_RAW`, `BZR_STDERR`, and `BZR_EXIT`; define
`GAP_COUNT`, `LAST_TEST_RESULT`,
`run_pybz <args...>`, `pybz_sidecar_start <runtime> <bugzilla-container>`,
`pybz_sidecar_stop <runtime>`, and `expect_gap <issue>`. The runner in Task 3 sets
`FUNC_CONFIG_DIR`, `CURRENT_TEST_GROUP`, and the active Bugzilla version before calling them.

**Verification**

- Mode: focused-test — expected-gap state transitions; add a shell fixture that sources the library,
  simulates pass and fail outcomes, drives the actual terminal and GitHub summary path, and asserts
  an expected-gap-only result exits zero with all four counters while a stale gap exits non-zero;
  red is a missing `expect_gap`; green command is the new fixture script with exit 0.
- Mode: focused-test — existing assertion reuse after `run_pybz`; a container-backed fixture runs a
  successful and failing CLI call then invokes `assert_success`/`assert_failure` against the shared
  BZR capture globals; red is assertions reading stale bzr output; green is the sidecar container
  fixture with exit 0.
- Mode: focused-test — sidecar image contains python-bugzilla 3.3.0 and its CLI; container smoke test
  builds the image and checks `python -c` package metadata plus `bugzilla --version`; red is a missing
  image; green command is `bash tests/functional/pybz/container-tests.sh`, expected exit 0.
- Mode: focused-test — SELinux-compatible exchange mount; the container fixture writes through the
  `/work:Z` bind and verifies the bytes from the host; red is a permission error or missing host
  file; green is the same container fixture with exit 0.

**Steps**

1. Add the expected-gap fixture and observe its missing-function failure.
2. Add GAP state tracking and `expect_gap`, including decimal issue validation, one-use enforcement,
   PASS→FAIL stale-marker behavior, and FAIL→GAP behavior; rerun the fixture to green.
3. Add the Containerfile with pinned `python:3.14.7-slim-bookworm` and
   `python-bugzilla==3.3.0`, using no RUN heredoc, plus a literal long-lived command.
4. Add the container smoke fixture, build it with the detected runtime, and verify package/CLI
   versions.
5. Implement checkout-scoped image naming, sidecar naming, image build, network-namespace join,
   `/work` bind mount, persistent home volume, shared BZR capture, and targeted cleanup.
6. Run shellcheck and Bash syntax checks for the changed scripts, then commit.

**Acceptance:** The client is pinned and callable only inside the sidecar; output and outcome state
are captured; expected gaps fail when stale; cleanup targets only this checkout/version.

## Task 3: Build the comparison runner and first real phase

**Files:** create `tests/functional/run-compare.sh`,
`tests/functional/run-compare-all.sh`, and `tests/functional/compare/00-products.sh`; modify
`tests/functional/lib.sh` only for integration defects found by the real run.

**Interfaces:** The runner sets `TEST_ID_PREFIX=compare` and publishes `BZ_URL`, `BZR_BIN`, shared
BZR capture globals, and test lifecycle globals to sourced phases. `00-products.sh` snapshots the
first capture, calls `run_bzr --server-url "$BZ_URL" product list` from a fresh empty config and
`run_pybz --bugzilla http://127.0.0.1 info --products`, then compares normalized product-name files.
The all-version script invokes setup, comparison, and cleanup for bz50/bz52/bz53.

**Verification**

- Mode: focused-test — real product-list parity through both clients;
  `compare/00-products.sh` test ID `compare/00-products/list-products`; red is sidecar/client startup or
  normalized-list mismatch; green command is `make functional-compare`, expected one PASS, zero
  failures, and exit 0.
- Mode: focused-test — all supported Bugzilla versions; green command is
  `make functional-compare-all`, expected bz50/bz52/bz53 PASSED and exit 0.

**Steps**

1. Create the phase first, add it to the not-yet-existing runner contract, invoke the focused Make
   target, and observe the missing-target failure.
2. Implement runner setup, private exchange directory, sidecar lifecycle, canonical phase loop,
   summary, GitHub summary append, and cleanup trap.
3. Implement the sequential all-version driver, continuing after a failed version and preserving a
   non-zero aggregate result.
4. Add Make targets that build release bzr and start the selected server before comparison, then
   add the second semantic-ID checker invocation now that its runner and phase tree exist.
5. Run `make functional-compare`; inspect both captured clients on failure and correct only verified
   protocol/format mismatches.
6. Run `make functional-compare-all`; expect all three versions green, then commit.

**Acceptance:** One real semantic comparison passes through both clients for every supported server;
all four result classes are represented by the harness contract; cleanup runs on every exit.

## Task 4: Publish the maintained surface

**Files:** modify `.github/workflows/functional-tests.yml`; create
`docs/dev/python-bugzilla-parity.md`; update shell/test guard lists in `Makefile`.

**Interfaces:** The workflow comparison job invokes `make functional-compare-all` with the already
built release binary and uploads counts through `GITHUB_STEP_SUMMARY`. The parity report references
the stable ID `compare/00-products/list-products` and marks only that proven row as `parity`.

**Verification**

- Mode: focused-test — workflow and Make dependency graph; `make -n functional-compare-all` must
  resolve every target and show the three-version runner.
- Mode: task-test-not-applicable — parity prose has no executable consumer beyond the stable test ID;
  manual review checks the four required columns and the first evidence row without snapshotting
  wording.

**Steps**

1. Add the scheduled/manual comparison job with read-only permissions, release build reuse, a
   bounded timeout, and always-run targeted cleanup.
2. Extend `check-shell` to cover the runner, all-version driver, comparison phases, and pybz test
   scripts; extend `check-functional-test-ids` to validate the comparison tree.
3. Create the parity report skeleton and product-list `parity` row; leave later rows and their
   terminal classifications to their owning follow-on issues.
4. Run `make -n functional-compare-all`, then `make lint`, `make test`,
   `make functional-compare-all`, and `make functional-test-all`; expect exit 0 for each.
5. Commit documentation and integration wiring.

**Acceptance:** Local and scheduled/manual entry points are discoverable and guarded; the report
records only evidence the shipped comparison proves.

## Rollback and cleanup

Reverting this change removes the comparison runner, sidecar, CI job, and report without touching
compiled bzr behavior or existing functional targets. Every local/CI run removes sidecar containers
and temporary files; named cache volumes are intentionally reusable and can be removed explicitly by
their checkout/version-derived names.
