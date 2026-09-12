# RHBZ custom-field and sub-component catalogue plan

**Goal.** Add four real-RHBZ catalogue controls for #775 without adding bzr
features. The phase uses the existing adapter, private exchange directory, and
disposable RHBZ lifecycle after phases 07 and 08.

Tech stack: Bash, Python 3.14/python-bugzilla 3.3.0, jq, existing RHBZ image,
and the existing Rust binary.

## Global Constraints

- Rust 1.89.0 and the current-thread runtime are unchanged.
- Add no dependency, bzr command, configuration, schema, or transport logic.
- Keep RHBZ out of stock bz50/bz52/bz53 runners.
- Use `compare/09-rhbz-fields/{sub-components,target-release,fixed-in,whiteboards}`.
- A real RHBZ readback, not a proxy or recorder, decides every classification.

Expected implementation size: 180–300 changed lines (M) — one adapter extension,
one phase, runner and shell fixtures, and four report rows.

## File map

- Modify `tests/functional/compare/python-bugzilla-adapter.py` for bounded RHBZ
  argument operations.
- Add `tests/functional/compare/rhbz/09-rhbz-fields.sh` for fixtures and live
  readbacks.
- Modify `tests/functional/run-rhbz-compare.sh` and
  `tests/functional/pybz/container-tests.sh` for order and focused fixtures.
- Modify `docs/dev/python-bugzilla-parity.md` for four evidence rows.

## Task 1 — expose bounded Python-Bugzilla controls

**Interfaces.** The adapter consumes an API key, a positive bug ID, and the
named RHBZ values; it supplies existing `{"transport":...,"result":...}`
responses to phase 09.

**Verification.**

- Contract: each operation rejects unknown, missing, or wrongly typed fields
  before dispatch. Mode: focused-test. Red: the new fixture fails on the base
  because operations are absent. Green: `make test-one T=pybz_container_tests`
  passes and records the exact python-bugzilla arguments.

Steps: add one validator-backed operation for sub-component, target-release,
fixed-in, and the three whiteboards; register them with the existing adapter;
extend recording-backend fixtures for valid and malformed requests.

Acceptance: every forwarded argument uses python-bugzilla's documented RHBZ
name and no unknown request member reaches a backend.

## Task 2 — add the live RHBZ catalogue phase

**Interfaces.** `09-rhbz-fields.sh` consumes the phase-08 runner environment,
`resource_pybz`, `run_bugzilla_sql_file`, and the Task 1 operation names. It
produces exactly four stable IDs and one evidence-led classification per ID.

**Verification.**

- Contract: every positive control validates metadata/privilege, persists its
  value, and reads it back before probing bzr. Mode: focused-test. Red: a
  missing phase/ID/control fails the shell fixture. Green: `make test-one
  T=pybz_container_tests` passes.

Steps: source phase 09 after phase 08; create run-token bugs; check field
metadata and administrator controls; call each Task 1 operation; assert the
corresponding live REST fields; probe bzr; classify only the observed outcome;
test missing-control and diagnostic paths in the shell fixture.

Acceptance: no missing field, permission, or python-bugzilla failure is
reported as parity or an expected gap.

## Task 3 — publish evidence and prove the full route

**Interfaces.** The report consumes Task 2's four IDs and classifications.

**Verification.**

- Contract: four report rows name the four semantic IDs. Mode: focused-test.
  Red: fixture rejects a missing row/ID. Green: `make test-one
  T=pybz_container_tests` passes.

Steps: add the four rows; run `make lint`, `make test`, and
`make functional-compare-rhbz`; expect zero exits, phase 09 output for all
four IDs, and lifecycle cleanup.

Acceptance: the parity report has one evidence-led entry per required
capability, and the real RHBZ invocation completes without failures.
