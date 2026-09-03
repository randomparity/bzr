# Bug lifecycle comparison implementation plan

Goal: add semantic bzr/python-bugzilla comparisons for the five bug-lifecycle capabilities in
issue #667. The existing ADR 0044 sidecar remains the architecture; a shell phase orchestrates the
tests and a small Python adapter exposes python-bugzilla's library operations as JSON.

Tech stack: Bash, jq, Python 3.14, python-bugzilla 3.3.0, Docker/Podman, Make.

Expected implementation size: 220–360 changed lines (M) — derived from one Python adapter, one
five-test shell phase, runner wiring, deterministic fixture coverage, and five report rows.

## Global constraints

- Host architecture is arm64; declared targets are arm64, x86_64, powerpc64le, and s390x. The host
  is included, but passing locally does not replace multi-target CI.
- Reuse ADR 0044's sidecar and semantic-result contracts; add no host Python dependency.
- Compare persisted server fields, not presentation bytes or generated identifiers.
- Preserve the exact python-bugzilla 3.3.0 and Python 3.14.7 image versions.
- Stay within issue #667; issues #670–#672, #679–#680, and #683 retain their capabilities.
- Guardrails are `make lint`, `make test`, and `make functional-test-all`; the comparison-specific
  live proof is `make functional-compare-all`.
- BASE_BRANCH is `main`; branch is `feat/bug-lifecycle-comparison-667`.

## File map

- Create `tests/functional/compare/bug-lifecycle.py`: python-bugzilla JSON adapter.
- Create `tests/functional/compare/01-bug-lifecycle.sh`: lifecycle orchestration and comparisons.
- Modify `tests/functional/run-compare.sh`: credentials, helper execution environment, phase order.
- Modify `tests/functional/pybz/container-tests.sh`: deterministic lifecycle fixture and failure
  controls.
- Modify `Makefile`: include the Python helper in syntax/static checks if a configured Python check
  already exists; otherwise no change is required because the live fixture executes it.
- Modify `docs/dev/python-bugzilla-parity.md`: five evidence rows.

## Task 1: Add the python-bugzilla lifecycle adapter

### Interfaces

- Consumes: `/work` exchange paths, `http://127.0.0.1`, fixed API key supplied as an argument,
  python-bugzilla 3.3.0 `Bugzilla`, `build_createbug`, `build_query`, `build_update`, `createbug`,
  `query`, `update_bugs`, `getbug`, and `bugs_history_raw` interfaces verified from the installed
  comparison image.
- Provides: `bug-lifecycle.py OP INPUT OUTPUT`, where OUTPUT is one JSON object with `transport`
  and `result`; operations are `create`, `query`, `update`, `view`, and `history`.

### Verification

- Adapter dispatch and output schema — Mode: focused-test. Add the deterministic fixture in
  `tests/functional/pybz/container-tests.sh`; first observe failure because the adapter is absent,
  then expect `bash tests/functional/pybz/container-tests.sh` to exit 0.

### Steps

1. Add fixture cases that invoke the adapter with a fake in-process `bugzilla` module and assert
   each operation's JSON result plus a non-empty transport.
2. Run `bash tests/functional/pybz/container-tests.sh`; expect non-zero with the adapter missing.
3. Implement strict argument parsing, fixed operation dispatch, API-key connection, positive ID
   validation, JSON serialization, and backend-class transport reporting.
4. Run the focused fixture; expect exit 0.
5. Commit with `test(functional): add bug lifecycle adapter`.

## Task 2: Add the five semantic comparison tests

### Interfaces

- Consumes: Task 1's adapter, existing `run_bzr`, `run_pybz`, `test_begin`, `test_pass`,
  `test_fail`, `expect_gap`, exchange directory, and bzr's `--api rest` inline-server flags.
- Provides: stable IDs `compare/01-bug-lifecycle/create`, `/query`, `/update`, `/view`, and
  `/history`, plus transport evidence files for both clients.

### Verification

- Five lifecycle results and semantic normalization — Mode: focused-test. Extend
  `tests/functional/pybz/container-tests.sh` with a controlled sidecar/runtime and bzr fixture;
  first observe missing phase IDs, then expect the fixture command to exit 0 with all five passes.
- Mismatch detection — Mode: focused-test. Perturb one normalized persisted field in the controlled
  fixture, expect the fixture command to exit non-zero and name the affected stable ID.

### Steps

1. Add the deterministic phase fixture and mismatch control; run it and observe the expected red
   missing-phase failure.
2. Add runner constants for the disposable administrator identity/key and export only what the
   sourced phase requires.
3. Implement phase-local helpers for JSON request files, bzr REST invocation, sidecar adapter
   invocation, positive-ID extraction, canonical state projection, history projection, transport
   recording, and diff-based equality.
4. Implement create, query, update, view, and history tests in order, each failing immediately on a
   client or normalization error and using no speculative `expect_gap`.
5. Add `01-bug-lifecycle` to the explicit runner list and run the controlled fixture; expect all
   five IDs to pass.
6. Enable the mismatch control; expect a non-zero result naming the changed capability, then remove
   the fault and observe green again.
7. Run `make functional-compare-all`; expect bz50, bz52, and bz53 to pass and cleanup to remove all
   sidecars.
8. Commit with `test(functional): compare bug lifecycle clients`.

## Task 3: Publish evidence and verify the branch

### Interfaces

- Consumes: Task 2's exact stable IDs and live green results.
- Provides: parity report rows whose evidence links remain stable across presentation changes.

### Verification

- Report coverage — Mode: focused-test. Add a fixture assertion that each of the five phase IDs
  occurs exactly once in `docs/dev/python-bugzilla-parity.md`; first observe the missing-row
  failure, then expect the fixture command to exit 0.

### Steps

1. Add five parity rows for create, query, update, view, and history, each marked `parity` and tied
   to its exact stable test ID.
2. Run the focused fixture; expect exit 0 with exact row coverage.
3. Run `make lint`; expect exit 0.
4. Run `make test`; expect exit 0.
5. Run `make functional-compare-all`; expect all supported versions to pass.
6. Run `make functional-test-all`; expect all established functional phases to pass.
7. Commit with `docs(parity): record bug lifecycle evidence`.

## Rollback and cleanup

Reverting the commits removes the comparison phase, adapter, and report rows without changing
production behavior or persisted data. The runner's existing EXIT trap owns sidecar and exchange
cleanup on every command outcome; no task may add broad container or volume pruning.

