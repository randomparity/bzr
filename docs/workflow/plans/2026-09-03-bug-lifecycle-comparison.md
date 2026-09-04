# Bug lifecycle comparison implementation plan

Goal: add semantic bzr/python-bugzilla comparisons for the five common bug-lifecycle capabilities
and fail-closed baselines for five confirmed dependent gaps in issue #667. The existing ADR 0044
sidecar remains the architecture; a shell phase orchestrates the tests and a small Python adapter
exposes python-bugzilla's library operations as JSON.

Tech stack: Bash, jq, Python 3.14, python-bugzilla 3.3.0, Docker/Podman, Make.

Expected implementation size: 1,760–1,850 changed lines (L) — derived from the existing
1,709-line implementation/report delta plus the shared transport classifier, closed adapter
normalization, and focused false-green controls. The operator-authorized 1,850-line ceiling
applies to implementation changes; the specification, ADRs, and this plan are excluded from that
count.

## Global constraints

- Host architecture is arm64; declared targets are arm64, x86_64, powerpc64le, and s390x. The host
  is included, but passing locally does not replace multi-target CI.
- Reuse ADR 0044's sidecar and semantic-result contracts; add no host Python dependency.
- Follow ADR 0045: transport evidence comes only from request-boundary debug events or the pinned
  python-bugzilla backend class and normalizes to exactly `REST` or `XMLRPC`.
- Compare persisted server fields, not presentation bytes or generated identifiers.
- Preserve the exact python-bugzilla 3.3.0 and Python 3.14.7 image versions.
- Stay within issue #667's test/report baseline; issues #670–#672, #679–#680, and #683 retain
  their product capabilities.
- Guardrails are `make lint`, `make test`, and `make functional-test-all`; the comparison-specific
  live proof is `make functional-compare-all`.
- BASE_BRANCH is `main`; branch is `feat/bug-lifecycle-comparison-667`.

## Resume status

Tasks 1–4 were completed before the final whole-branch review exposed the self-asserted transport
defect. Their commits have not been pushed or delivered in a pull request. Task 5 is the corrective
continuation authorized by the expanded scope and is a hard gate before any delivery; its focused
controls and complete guardrail rerun replace the weaker transport evidence from the earlier task
checkpoints. The historical tasks remain in their executed order rather than pretending the
finding was known before review.

## File map

- Create `tests/functional/compare/bug-lifecycle.py`: fixed python-bugzilla JSON adapter for the
  common lifecycle and five live capability probes.
- Create `tests/functional/compare/01-bug-lifecycle.sh`: common lifecycle comparisons, five
  expected-gap baselines, and exact issue ownership.
- Modify `tests/functional/lib.sh`: shared private sidecar command capture boundary and thin fixed
  CLI/adapter wrappers, plus bzr request-boundary transport classification.
- Modify `tests/functional/run-compare.sh`: credentials, private helper staging, phase order.
- Create `tests/functional/compare/seed-saved-search.pl`: fixed, parameterized Bugzilla-container
  fixture for one run-specific administrator saved search on bz50, bz52, and bz53.
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
- Transport contract: use explicit REST for all bzr operations except #680's bug-tag mutation and
  filter, which use explicit XML-RPC. Record and assert the chosen transport per operation; require
  python-bugzilla's XML-RPC backend for #680.
- Saved-search fixture contract: Task 3 adds
  `seed_server_saved_search LOGIN NAME BUG_ID BUG_ID` to `tests/functional/run-compare.sh`. It
  validates the name and IDs, constructs `bug_id=<id>,<id>&bug_id_type=anyexact`, and invokes
  `seed-saved-search.pl LOGIN NAME QUERY` through the primary application container's
  `perl -I. -` standard input. The Perl helper resolves LOGIN through `Bugzilla::User`, writes only
  that user's NAME row with database placeholders, and verifies the exact stored owner/name/query.

### Verification

- Live gap behavior — Mode: focused-test. Extend
  `tests/functional/pybz/container-tests.sh` so each controlled python-bugzilla operation succeeds
  with the expected semantic result before its bzr attempt fails and becomes GAP; first observe
  missing gap IDs, then expect the fixture command to exit 0 with all five exact markers.
- Exact ownership and stale-gap failure — Mode: focused-test. In the same fixture, assert the
  complete ID-to-issue mapping and rerun each path with a passing, semantically equivalent bzr
  result; first observe the missing mapping assertions fail, then expect every stale marker to make
  the fixture exit non-zero and name its exact owning issue.
- Discriminating preconditions — Mode: focused-test and live-functional. Seed a run-specific
  saved search whose stored query selects exactly two controlled positive bug IDs and verify it in
  each supported Bugzilla image. Require #679's substring control to return both the exact and
  near-match decoy IDs while `equals` returns only the exact ID. Require #672's live comment-tag
  readback and controlled `minor_update: true` request shape independently. Require #680's
  XML-RPC transport records and fail the fixture when the probe inherits REST.

### Steps

1. Add fixture cases for the five stable IDs, their complete issue mapping, successful live-client
   precondition, and a control that changes each bzr result from gap to semantic parity; run the
   fixture and observe non-zero because the phase IDs are absent.
2. Add `seed_server_saved_search LOGIN NAME BUG_ID BUG_ID` and the Perl seed helper, then add the
   saved-search probe. After both lifecycle bugs exist, derive a non-empty run-specific name of at
   most 64 characters and call the function with the disposable administrator and the two positive
   IDs. It constructs `bug_id=<id>,<id>&bug_id_type=anyexact`, streams the fixed script to
   `perl -I. -` in the primary application container, resolves the administrator through Bugzilla,
   inserts or updates one `namedqueries` row using placeholders, and reads back the exact owner,
   name, and query before proceeding. Run that name through python-bugzilla and require the sorted
   IDs to equal the controlled pair, attempt `bzr bug search --saved-search <name>`, compare the
   same exact set when the command succeeds, then route owner 670 through Task 5's terminal gap
   classifier. Exercise setup and
   readback on bz50, bz52, and bz53 rather than assuming an image-provided search. Standard-input
   execution leaves no helper or query file in the application container.
3. Add the arbitrary-fields probe: use python-bugzilla's generic field map to create and update a
   controlled bug, verify both persisted values, attempt equivalent repeatable `--field` bzr create
   and update operations, compare persisted values when they succeed, then route owner 671 through
   Task 5's terminal gap classifier.
4. Add #672's two evidence arms under the single `update-options` ID. In the live arm, post an
   update with a tagged comment through python-bugzilla and read the tag back. In the pinned-image
   controlled backend, separately require the outgoing update to contain `minor_update: true`, and
   require the live server to accept that same option. The bzr stale-gap substitute passes only
   when its controlled request also contains `minor_update: true` and its live comment tag matches;
   then route owner 672 through Task 5's terminal gap classifier. Add no mail or notification
   fixture.
5. Add the query-match-types probe: seed an exact run-specific whiteboard value and a second bug
   whose value appends a suffix. Prove the ordinary substring query returns both IDs, then query
   through python-bugzilla with `status_whiteboard_type=equals` and require only the exact ID.
   Attempt `bzr bug list --status-whiteboard-type equals`, require the same singleton on success,
   then route owner 679 through Task 5's terminal gap classifier.
6. Add the bug-tags probe: add a run-specific personal tag and query it through python-bugzilla,
   require its reported backend to be XML-RPC, and assert the controlled ID is returned. Attempt
   equivalent `bzr --api xmlrpc bug tag` and `bzr --api xmlrpc bug list --tag` operations, require
   XML-RPC transport evidence plus matching tags and IDs on success, then route owner 680 through
   Task 5's terminal gap classifier.
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

## Task 5: Replace asserted transport labels with observed evidence

### Interfaces

- Consumes: bzr's existing debug events `API response` from `BugzillaClient::send_raw` and
  `XML-RPC call` from `XmlRpcClient::call_with_status_policy`; python-bugzilla 3.3.0's concrete
  `_BackendREST` and `_BackendXMLRPC` class names; the existing `BZR_STDERR` capture.
- Provides: `observe_bzr_transport`, which sets `BZR_TRANSPORT` to exactly `REST` or `XMLRPC` only
  when a successful invocation's captured request-boundary events identify one class;
  `_transport(client)`, which returns the same closed values for the two pinned backend classes;
  lifecycle `*.transport` evidence copied from those results. `lifecycle_bzr_no_dispatch` owns only
  the successful #672 dry-run control and writes no transport record. A probe-specific recognized
  parser rejection requires exit 2 plus the exact unsupported option or subcommand diagnostic and
  writes no record. A probe-specific terminal classifier sets `LIFECYCLE_GAP_ELIGIBLE` only after
  a recognized rejection, successful observed client operations with structurally valid response
  evidence, or the dedicated successful dry-run with a structurally valid request payload.
  `lifecycle_expect_gap` calls `expect_gap` only in that state. Observation defects, malformed
  evidence, harness failures, and every other non-zero outcome remain FAIL.

### Verification

- Bzr boundary classification — Mode: focused-test. Extend `run_lifecycle_phase_fixture` so its
  fake bzr boundary emits REST or XML-RPC debug markers independently of semantic output. Before
  implementation, a #680 semantic-success/observed-REST control incorrectly becomes parity; after
  implementation, `bash tests/functional/pybz/container-tests.sh` exits 0 and shows that control
  as GAP. Missing and mixed-event controls after successful client invocations must remain FAIL
  rather than being converted to GAP. A recognized unsupported command must exit 2, name its exact
  probed option or subcommand, produce no transport record, and may remain an expected capability
  gap. Connection-style no-event failure and server/command-error controls must remain FAIL. The
  successful #672 dry-run must use the dedicated no-dispatch path and produce no transport claim;
  a live operation with missing events must still fail.
- Python backend normalization — Mode: focused-test. Give the adapter fixture concrete
  `_BackendREST` and `_BackendXMLRPC` backends plus an unknown backend case. Before implementation,
  the unknown class is emitted as transport; after implementation, the same focused command exits
  0 only when the adapter rejects it and every successful result contains exactly `REST` or
  `XMLRPC`.

### Steps

1. Add the semantic-success/observed-REST #680 control; missing/mixed successful-client controls;
   connection-style no-event and server/command-error controls; a recognized exit-2 parser-gap
   control; the dedicated successful #672 dry-run no-dispatch control; exact closed-value
   assertions; and unknown python-backend rejection to
   `tests/functional/pybz/container-tests.sh`.
2. Run `bash tests/functional/pybz/container-tests.sh`; expect non-zero because the current wrappers
   still self-assert bzr transport and the adapter still accepts unknown backend names.
3. In `tests/functional/lib.sh`, add `BZR_TRANSPORT` and `observe_bzr_transport`: inspect
   `BZR_STDERR` for the two established debug messages after successful client operations. Accept
   one or more observations of exactly one class and return non-zero with an actionable diagnostic
   for neither or both. Do not inspect CLI arguments or URLs.
4. In `tests/functional/compare/01-bug-lifecycle.sh`, invoke ordinary bzr wrappers with
   `RUST_LOG=bzr=debug`, classify every successful invocation expected to exercise a client
   boundary, copy only `BZR_TRANSPORT`, remove the transport parameter and self-written defaults,
   and compare transport records by exact equality. Add a dedicated no-dispatch wrapper used only
   by #672's successful dry-run request-shape arm. Positively recognize each unsupported probe only
   from exit 2 plus its exact option/subcommand diagnostic. Add a probe-terminal classifier that
   sets `LIFECYCLE_GAP_ELIGIBLE` only for that rejection, successful observed client operations
   with structurally valid response evidence, or the dedicated successful dry-run with a
   structurally valid request payload. Route every marker through `lifecycle_expect_gap`, which
   refuses conversion outside that state. Every other non-zero outcome, malformed evidence,
   harness failure, and every missing or mixed observation remains FAIL.
5. In `tests/functional/compare/bug-lifecycle.py`, map `_BackendREST` to `REST` and
   `_BackendXMLRPC` to `XMLRPC`; raise `AdapterError` for missing or unknown classes. Update the
   fake backend names and expected closed outputs in the focused fixture.
6. Run `bash tests/functional/pybz/container-tests.sh`; expect exit 0 with every new control proven.
7. Run `make lint`, `make test`, `make functional-compare-all`, and `make functional-test-all`;
   expect exit 0 from each command.
8. Commit with `fix(functional): observe lifecycle comparison transports`.

## Rollback and cleanup

Reverting the commits removes the comparison phase, adapter, and report rows without changing
production behavior or persisted data. The runner's existing EXIT trap owns sidecar and exchange
cleanup on every command outcome; no task may add broad container or volume pruning.
