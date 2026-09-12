# RHBZ ExternalBugs catalogue implementation plan

Add a real-RHBZ catalogue phase for the three ExternalBugs mutations and
component update, keeping the python-bugzilla adapter as the operation
boundary and classifying bzr's absent surfaces as expected gaps. The shell
phase runs only after the RHBZ smoke phase; adapter tests and parity-report
fixtures verify its static contracts.

Tech stack: Bash functional phases, Python 3.14/python-bugzilla 3.3.0, jq,
the existing disposable RHBZ container, and Rust's existing CLI binary.

## Global Constraints

- Rust 1.89.0 and the current-thread runtime are unchanged.
- Do not add dependencies, bzr commands, config, schemas, or transport logic.
- Keep RHBZ out of stock bz50/bz52/bz53 comparison runners.
- Use stable `compare/08-rhbz-externalbugs/*` IDs and private exchange files.
- Real RHBZ behavior, not a proxy or local recorder, is the acceptance proof.

Expected implementation size: 180–280 changed lines (M) — one adapter extension,
one RHBZ phase, runner/fixture coverage, and four report rows.

## File map

- Modify `tests/functional/compare/python-bugzilla-adapter.py` for four
  validated real-server operations.
- Add `tests/functional/compare/rhbz/08-rhbz-externalbugs.sh` for fixtures,
  persisted-state assertions, and controlled bzr gaps.
- Modify `tests/functional/run-rhbz-compare.sh` to source phase 08 after smoke.
- Modify `tests/functional/pybz/container-tests.sh` for adapter, phase, and
  stable-ID fixtures.
- Modify `docs/dev/python-bugzilla-parity.md` for the four evidence rows.

## Task 1 — Expose validated python-bugzilla operations

Files: `tests/functional/compare/python-bugzilla-adapter.py`,
`tests/functional/pybz/container-tests.sh`.

Interfaces: each adapter operation consumes an object containing `api_key` and
the operation's bounded IDs/text; it returns the existing
`{"transport": ..., "result": ...}` envelope. The phase depends on operation
names `externalbugs_add`, `externalbugs_update`, `externalbugs_remove`, and
`component_update`.

Verification:

- Mode: focused-test. Contract: malformed request keys and values are rejected
  before dispatch, while each valid operation reaches its matching
  python-bugzilla backend method. Expected red: a wrong operation/malformed
  request makes the fixture fail. Green command: `make test-one
  T=pybz_container_tests`; expected result: the fixture suite passes.

Steps:

1. Add strict request validators and operation functions that call
   `Bugzilla.add_external_tracker`, `update_external_tracker`,
   `remove_external_tracker`, and `editcomponent`.
2. Register the operations as network-backed adapter operations, preserving
   existing envelope and transport observation rules.
3. Extend the container fixture with recording backends that assert the exact
   normalized parameter mappings and malformed-input failures.

Acceptance criteria: no operation accepts unknown fields; valid operation
fixtures prove their library method and transport envelope.

## Task 2 — Add the RHBZ catalogue phase

Files: `tests/functional/compare/rhbz/08-rhbz-externalbugs.sh`,
`tests/functional/run-rhbz-compare.sh`, `tests/functional/pybz/container-tests.sh`.

Interfaces: phase 08 consumes the smoke-established `BZ_URL`, the comparison
resource helpers, and the four adapter names from Task 1. It produces exactly
the four stable test IDs and one controlled `expect_gap 774` outcome for each.

Verification:

- Mode: focused-test. Contract: the runner sources phase 08 only after smoke;
  the phase preserves the four IDs and rejects missing fixture evidence or an
  unexpected bzr parser outcome. Expected red: a missing ID or failed positive
  control fails the shell fixture. Green command: `make test-one
  T=pybz_container_tests`; expected result: the fixture suite passes.

Steps:

1. Seed a run-token-scoped product, component, bug, and minimal external
   tracker configuration using the RHBZ test fixture convention.
2. Add the add/update/remove tests, reading the external-bug state after every
   python-bugzilla operation.
3. Add the component-update test, reading the changed component state after
   `editcomponent`.
4. Probe each absent bzr surface only for its controlled parser diagnostic,
   then apply `resource_expect_gap 774`.
5. Source phase 08 after smoke and extend shell fixtures for ordering, IDs,
   positive controls, and gap ownership.

Acceptance criteria: every row's python-bugzilla positive control is real RHBZ
state and every bzr absence is classified only by the controlled diagnostic.

## Task 3 — Publish evidence rows and run proof

Files: `docs/dev/python-bugzilla-parity.md` and the files from Tasks 1–2.

Interfaces: report rows consume the stable IDs from Task 2 and name #774 as
their expected-gap owner; no public bzr interface is added.

Verification:

- Mode: focused-test. Contract: the report has one evidence row for each
  stable ID. Expected red: a missing/old row makes the parity-report fixture
  fail. Green command: `make test-one T=pybz_container_tests`; expected result:
  the fixture suite passes.

Steps:

1. Replace the local-only component-update row with four real-RHBZ rows.
2. Run `make lint` and `make test`; expected result: both return zero.
3. Run `make functional-compare-rhbz`; expected result: smoke and all four
   catalogue IDs run without failures and lifecycle cleanup succeeds.

Acceptance criteria: the report has no stale #675 component-update evidence,
and the real RHBZ command proves all four controls.
