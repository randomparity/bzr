# Python-bugzilla comparison harness implementation plan

**Goal:** Run bzr and python-bugzilla 3.3.0 against the same real Bugzilla instances and report
semantic comparison outcomes.

**Architecture:** A pinned long-lived python-bugzilla sidecar joins each functional Bugzilla
container's network namespace. A dedicated runner reuses the functional test library, sources a
separate comparison phase tree, and reports pass/fail/skip/expected-gap counts.

**Tech stack:** Bash 3-compatible scripts, Docker/Podman CLI, Make, GitHub Actions, Python slim
container with `python-bugzilla==3.3.0`, jq, rg, shellcheck.

**Expected implementation size: 420–700 changed lines (L) — derived from four shell/runtime
contracts, guard fixtures, Make/CI wiring, one phase, and one report.**

## Global constraints

- Support Bugzilla versions bz50, bz52, and bz53 and host architectures declared by the repository:
  x86_64/aarch64/powerpc64le/s390x Linux, aarch64 macOS, and x86_64/aarch64 Windows.
- Preserve Bash 3 compatibility and BSD/GNU userland portability; use existing Docker/Podman
  detection and checkout-scoped identity helpers.
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
`tests/functional/compare`. Task 3 supplies that runner and directory.

**Verification**

- Mode: focused-test — alternate runner/phase parameters and source-directory validation;
  `tools/check-functional-test-ids-tests.sh`; red result is rejection of a valid alternate tree or
  acceptance of a mismatched source; green command is
  `bash tools/check-functional-test-ids-tests.sh`, expected exit 0 and its pass summaries.

**Steps**

1. Extend fixture helpers to create an alternate runner and phase tree, then assert valid custom
   paths pass and a mismatched source directory fails.
2. Run `bash tools/check-functional-test-ids-tests.sh`; expect the new valid fixture to fail because
   the checker treats the second argument as unsupported or ignores it.
3. Parse optional runner and phase-directory paths relative to the repository root, validate they
   remain distinct inputs, and derive the expected source directory basename for the canonical-loop
   parser.
4. Run the fixture script; expect exit 0 with both runtime and ID checker pass lines.
5. Wire both checker invocations into `check-functional-test-ids` and commit.

**Acceptance:** Existing default calls behave unchanged; custom runner/tree validation is strict and
covered by fixtures.

## Task 2: Add comparison result and sidecar primitives

**Files:** modify `tests/functional/lib.sh`; create
`tests/functional/pybz/Containerfile` and `tests/functional/pybz/container-tests.sh`.

**Interfaces:** Define `PYBZ_STDOUT`, `PYBZ_STDERR`, `PYBZ_EXIT`, `GAP_COUNT`,
`run_pybz <args...>`, `pybz_sidecar_start <runtime> <bugzilla-container>`,
`pybz_sidecar_stop <runtime>`, and `expect_gap <issue>`. The runner in Task 3 sets
`FUNC_CONFIG_DIR`, `CURRENT_TEST_GROUP`, and the active Bugzilla version before calling them.

**Verification**

- Mode: focused-test — expected-gap state transitions; add a shell fixture that sources the library,
  simulates the pass and fail outcomes, and asserts counter changes and stale-gap failure; red is a
  missing `expect_gap`; green command is the new fixture script with exit 0.
- Mode: focused-test — sidecar image contains python-bugzilla 3.3.0 and its CLI; container smoke test
  builds the image and checks `python -c` package metadata plus `bugzilla --version`; red is a missing
  image; green command is `bash tests/functional/pybz/container-tests.sh`, expected exit 0.

**Steps**

1. Add the expected-gap fixture and observe its missing-function failure.
2. Add GAP state tracking and `expect_gap`, including decimal issue validation, one-use enforcement,
   PASS→FAIL stale-marker behavior, and FAIL→GAP behavior; rerun the fixture to green.
3. Add the Containerfile with a pinned Python base and `python-bugzilla==3.3.0`, using no RUN
   heredoc, plus a literal long-lived command.
4. Add the container smoke fixture, build it with the detected runtime, and verify package/CLI
   versions.
5. Implement sidecar naming, image build, network-namespace join, `/work` bind mount, persistent
   home volume, command capture, and targeted cleanup.
6. Run shellcheck and Bash syntax checks for the changed scripts, then commit.

**Acceptance:** The client is pinned and callable only inside the sidecar; output and outcome state
are captured; expected gaps fail when stale; cleanup targets only this checkout/version.

## Task 3: Build the comparison runner and first real phase

**Files:** create `tests/functional/run-compare.sh`,
`tests/functional/run-compare-all.sh`, and `tests/functional/compare/00-products.sh`; modify
`tests/functional/lib.sh` only for integration defects found by the real run.

**Interfaces:** The runner publishes `BZ_URL`, `BZR_BIN`, `PYBZ_*`, and test lifecycle globals to
sourced phases. `00-products.sh` calls `run_bzr product list` and
`run_pybz --bugzilla http://127.0.0.1 info --products`, then compares normalized product-name files.
The all-version script invokes setup, comparison, and cleanup for bz50/bz52/bz53.

**Verification**

- Mode: focused-test — real product-list parity through both clients;
  `compare/00-products.sh` test ID `00-products/list-products`; red is sidecar/client startup or
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
4. Add Make targets that build release bzr and start the selected server before comparison.
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
the stable ID `00-products/list-products`.

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
3. Create the parity report skeleton and product-list row.
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
