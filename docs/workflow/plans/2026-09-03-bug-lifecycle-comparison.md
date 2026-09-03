# Bug lifecycle comparison implementation plan

Goal: add semantic bzr/python-bugzilla comparisons for the five common bug-lifecycle capabilities
and fail-closed baselines for five confirmed dependent gaps in issue #667. The existing ADR 0044
sidecar remains the architecture; a shell phase orchestrates the tests and a small Python adapter
exposes python-bugzilla's library operations as JSON.

Tech stack: Bash, jq, Python 3.14, python-bugzilla 3.3.0, Docker/Podman, Make.

Expected implementation size: 380–600 changed lines (M) — derived from one Python adapter, one
ten-test shell phase, runner wiring, deterministic parity and stale-gap fixtures, and ten report
rows.

## Global constraints

- Host architecture is arm64; declared targets are arm64, x86_64, powerpc64le, and s390x. The host
  is included, but passing locally does not replace multi-target CI.
- Reuse ADR 0044's sidecar and semantic-result contracts; add no host Python dependency.
- Compare persisted server fields, not presentation bytes or generated identifiers.
- Preserve the exact python-bugzilla 3.3.0 and Python 3.14.7 image versions.
- Stay within issue #667's test/report baseline; issues #670–#672, #679–#680, and #683 retain
  their product capabilities.
- Guardrails are `make lint`, `make test`, and `make functional-test-all`; the comparison-specific
  live proof is `make functional-compare-all`.
- BASE_BRANCH is `main`; branch is `feat/bug-lifecycle-comparison-667`.

## File map

- Create `tests/functional/compare/bug-lifecycle.py`: fixed python-bugzilla JSON adapter for the
  common lifecycle and five live capability probes.
- Create `tests/functional/compare/01-bug-lifecycle.sh`: common lifecycle comparisons, five
  expected-gap baselines, and exact issue ownership.
- Modify `tests/functional/lib.sh`: shared private sidecar command capture boundary and thin fixed
  CLI/adapter wrappers.
- Modify `tests/functional/run-compare.sh`: credentials, private helper staging, phase order.
- Modify `tests/functional/pybz/container-tests.sh`: deterministic lifecycle, gap ownership,
  stale-gap, and failure controls.
- Modify `docs/dev/python-bugzilla-parity.md`: five parity rows and five expected-gap rows.

## Task 1: Add the python-bugzilla lifecycle adapter

### Interfaces

- Consumes: `/work` exchange paths, `http://127.0.0.1`, fixed API key inside a mode-private JSON
  request file,
  python-bugzilla 3.3.0 `Bugzilla`, `build_createbug`, `build_query`, `build_update`, `createbug`,
  `query`, `update_bugs`, `getbug`, and `bugs_history_raw` interfaces verified from the installed
  comparison image.
- Provides: `bug-lifecycle.py OP INPUT OUTPUT`, where INPUT contains the API key plus operation
  parameters and OUTPUT is one JSON object with `transport` and `result`; operations are `create`,
  `query`, `update`, `view`, `history`, `saved_search`, `generic_fields`, `update_options`,
  `match_type`, and `bug_tags`. Each gap operation accepts only the fields needed by its fixed live
  probe.

### Verification

- Adapter syntax, dispatch, and output schema — Mode: focused-test. Add the deterministic fixture
  in `tests/functional/pybz/container-tests.sh` and run it inside the pinned comparison image;
  first observe failure because the adapter is absent, then expect
  `bash tests/functional/pybz/container-tests.sh` to exit 0 without requiring host Python.

### Steps

1. Add fixture cases that invoke the adapter inside the pinned comparison image with a fake
   in-process `bugzilla` module and assert syntax loading, every fixed operation's JSON result, and
   a non-empty transport.
2. Run `bash tests/functional/pybz/container-tests.sh`; expect non-zero with the adapter missing.
3. Implement strict argument parsing, fixed operation dispatch, API-key loading without printing
   secret content, positive ID validation, JSON serialization, and backend-class transport
   reporting.
4. Run the focused fixture; expect exit 0.
5. Commit with `test(functional): add bug lifecycle adapter`.

## Task 2: Add the five semantic comparison tests

### Interfaces

- Consumes: Task 1's adapter, existing `run_bzr`, new fixed `run_pybz_adapter`, `test_begin`,
  `test_pass`, `test_fail`, `expect_gap`, exchange directory, and bzr's `--api rest` inline-server
  flags. `_run_pybz_command COMMAND [ARG ...]` is private; `run_pybz` passes fixed `bugzilla`, and
  `run_pybz_adapter` passes fixed `python /work/compare/bug-lifecycle.py` before caller arguments.
- Provides: stable IDs `compare/01-bug-lifecycle/create`, `/query`, `/update`, `/view`, and
  `/history`, plus transport evidence files for both clients. Create description parity is read
  through `bzr comment list` with explicit REST selection for both generated IDs.

### Verification

- Sidecar capture compatibility — Mode: focused-test. Extend
  `tests/functional/pybz/container-tests.sh` with a fake runtime that asserts both thin wrappers
  select their fixed commands and preserve stdout, raw stdout, stderr, and exit status; first
  observe failure because the adapter wrapper is absent, then expect the fixture command to exit 0.
- Five lifecycle results, first-comment description reads, and semantic normalization — Mode:
  focused-test. Extend `tests/functional/pybz/container-tests.sh` with a controlled
  sidecar/runtime and bzr fixture; require history normalization to rewrite only an old `summary`
  value exactly equal to either controlled client summary while preserving other values and
  ordering. First observe missing phase IDs, then expect the fixture command to exit 0 with all
  five passes.
- Mismatch detection — Mode: focused-test. Perturb one normalized persisted field in the
  controlled fixture, expect the fixture command to exit non-zero and name the affected stable ID.

### Steps

1. Add the deterministic phase fixture and mismatch control; run it and observe the expected red
   missing-phase failure.
2. Add runner constants for the disposable administrator identity/key, set umask 077, copy the
   adapter to `$COMPARE_EXCHANGE_DIR/bug-lifecycle.py` before sidecar startup, fail on missing or
   unreadable staging, and export only what the sourced phase requires.
3. Extract the existing sidecar execution and capture logic into private
   `_run_pybz_command COMMAND [ARG ...]`; make thin `run_pybz` and `run_pybz_adapter` wrappers call
   it with their fixed commands. Implement phase-local helpers for mode-private JSON request
   files, bzr REST invocation, positive-ID extraction, canonical state projection, history
   projection, transport recording, and diff-based equality. Define each initial summary as a
   shared run-specific stem plus exact ` [bzr]` or ` [pybz]` suffix; history projection maps only
   exact matches for those two old summary values to the stem and otherwise preserves the record.
4. Implement create, query, update, view, and history tests in order. The create test reads each
   bug's first comment with the same forced-REST bzr observer before comparing descriptions. Each
   test fails immediately on a client or normalization error and uses no speculative `expect_gap`.
5. Add `01-bug-lifecycle` to the explicit runner list and run the controlled fixture; expect all
   five IDs to pass.
6. Enable the mismatch control; expect a non-zero result naming the changed capability, then remove
   the fault and observe green again.
7. Run `make functional-compare-all`; expect bz50, bz52, and bz53 to pass and cleanup to remove all
   sidecars.
8. Commit with `test(functional): compare bug lifecycle clients`.

## Task 3: Establish the five owned expected-gap baselines

### Interfaces

- Consumes: Task 1's fixed gap operations; Task 2's request, normalization, transport, and result
  helpers; `expect_gap <positive-decimal-issue>` from the existing harness.
- Provides: stable IDs `compare/01-bug-lifecycle/saved-search`, `/arbitrary-fields`,
  `/update-options`, `/query-match-types`, and `/bug-tags`, mapped respectively and exclusively to
  #670, #671, #672, #679, and #680.

### Verification

- Live gap behavior — Mode: focused-test. Extend
  `tests/functional/pybz/container-tests.sh` so each controlled python-bugzilla operation succeeds
  with the expected semantic result before its bzr attempt fails and becomes GAP; first observe
  missing gap IDs, then expect the fixture command to exit 0 with all five exact markers.
- Exact ownership and stale-gap failure — Mode: focused-test. In the same fixture, assert the
  complete ID-to-issue mapping and rerun each path with a passing, semantically equivalent bzr
  result; first observe the missing mapping assertions fail, then expect every stale marker to make
  the fixture exit non-zero and name its exact owning issue.

### Steps

1. Add fixture cases for the five stable IDs, their complete issue mapping, successful live-client
   precondition, and a control that changes each bzr result from gap to semantic parity; run the
   fixture and observe non-zero because the phase IDs are absent.
2. Add the saved-search probe: run the administrator's `My Bugs` server-side saved search through
   python-bugzilla, validate its JSON result set, attempt `bzr bug search --saved-search "My Bugs"`,
   compare results when the command succeeds, then apply `expect_gap 670` to the current failure.
3. Add the arbitrary-fields probe: use python-bugzilla's generic field map to create and update a
   controlled bug, verify both persisted values, attempt equivalent repeatable `--field` bzr create
   and update operations, compare persisted values when they succeed, then apply `expect_gap 671`.
4. Add the update-options probe: update a controlled bug with `minor_update` and a tagged comment
   through python-bugzilla, read back the comment tag, attempt equivalent bzr
   `--minor-update --comment-tag` behavior, compare the tag when it succeeds, then apply
   `expect_gap 672`.
5. Add the query-match-types probe: seed a unique whiteboard value, query it through
   python-bugzilla with the exact-match modifier, assert only the controlled ID is returned, attempt
   `bzr bug list --status-whiteboard-type equals`, compare IDs when it succeeds, then apply
   `expect_gap 679`.
6. Add the bug-tags probe: add a run-specific personal tag and query it through python-bugzilla,
   assert the controlled ID is returned, attempt equivalent `bzr bug tag` and `bug list --tag`
   operations, compare tags and IDs when they succeed, then apply `expect_gap 680`.
7. Run the focused fixture; expect exit 0 with five GAP outcomes and exact issue ownership. Enable
   the stale-gap control; expect non-zero results naming #670, #671, #672, #679, and #680, then
   remove the control and observe green again.
8. Run `make functional-compare-all`; expect the five parity results and five expected gaps to pass
   on bz50, bz52, and bz53 with cleanup removing every sidecar.
9. Commit with `test(functional): baseline bug lifecycle gaps`.

## Task 4: Publish evidence and verify the branch

### Interfaces

- Consumes: Task 2's five parity IDs and Task 3's five owned expected-gap IDs and live results.
- Provides: parity report rows whose evidence links remain stable across presentation changes.

### Verification

- Report coverage — Mode: focused-test. Add a fixture assertion that all ten phase IDs occur
  exactly once in `docs/dev/python-bugzilla-parity.md`, parity IDs have status `parity`, and each
  expected gap row names exactly its mapped owner; first observe the missing-row failure, then
  expect the fixture command to exit 0.

### Steps

1. Add five parity rows for create, query, update, view, and history, each marked `parity`, plus
   five expected-gap rows tied exactly to #670, #671, #672, #679, and #680 and their stable IDs.
2. Run the focused fixture; expect exit 0 with exact row coverage.
3. Run `make lint`; expect exit 0 without a host-Python prerequisite.
4. Run `make test`; expect exit 0.
5. Run `make functional-compare-all`; expect all supported versions to pass.
6. Run `make functional-test-all`; expect all established functional phases to pass.
7. Commit with `docs(parity): record bug lifecycle evidence`.

## Rollback and cleanup

Reverting the commits removes the comparison phase, adapter, and report rows without changing
production behavior or persisted data. The runner's existing EXIT trap owns sidecar and exchange
cleanup on every command outcome; no task may add broad container or volume pruning.
