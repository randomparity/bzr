# RHBZ field transition proof implementation plan

**Goal.** Make #795's four RHBZ field comparisons prove independent persisted
python-bugzilla and bzr mutations, including structured sub-component input.

The existing RHBZ phase remains the owner of disposable bug state. Its fixture
models REST readback; bzr's existing command entry points remain the only bzr
surface. The test fixture and parity matrix respectively own executable proof
and published classification. Bash, jq, python-bugzilla 3.3.0, the existing
RHBZ image, and Rust 1.89.0 are unchanged.

## Global Constraints

- Add no dependency, bzr command, configuration, schema, or transport logic.
- Keep RHBZ coverage out of stock bz50/bz52/bz53 runners.
- Use `compare/09-rhbz-fields/{sub-components,target-release,fixed-in,whiteboards}`.
- A real RHBZ readback, not a proxy or recorder, decides a live classification.
- Preserve issue #795 exclusions: #663, #774, and #796 own their stated work.

Expected implementation size: 120–220 changed lines (M) — phase state
separation, fixture controls, and four report rows.

## File map

- `tests/functional/compare/rhbz/09-rhbz-fields.sh` owns live mutation order
  and REST/bzr readbacks; extend it with three-value inputs and JSON input.
- `tests/functional/pybz/container-tests.sh` owns modeled command/readback
  controls; extend its RHBZ fields fixture for distinct bzr values and no-op.
- `docs/dev/python-bugzilla-parity.md` owns published classification; replace
  stale predicted-gap wording with the executed bzr field surfaces.

## Task 1 — prove independent RHBZ transitions

**Files:** modify `tests/functional/compare/rhbz/09-rhbz-fields.sh`.

**Interfaces:** consume existing `resource_pybz`, `run_bzr`, `run_bzr_raw`,
`rhbz_fields_read`, and `rhbz_fields_read_with_bzr`; produce the same four
stable IDs and their observed classification for Task 3.

**Verification.**

- Contract: a bzr write must differ from and read after the python-bugzilla
  write for each stable ID. Mode: focused-test. Red: fixture mode that accepts
  successful bzr no-op reports four failures. Green: `bash
  tests/functional/pybz/container-tests.sh` passes.
- Contract: sub-components use JSON object input and response `sub_components`.
  Mode: focused-test. Red: fixture rejects a scalar or missing JSON input.
  Green: same command passes its structured-input assertions.
- Contract: bzr whiteboards persist devel/internal/QA. Mode: focused-test.
  Red: a single-field mutation fails the modeled readback. Green: same command
  passes all three values.

**Steps.**

1. Add per-arm before/reference/bzr values generated from the run token. For
   target release, start the bug empty and insert/read back distinct configured
   reference and bzr release rows before either write, then assert REST state
   after the python-bugzilla reference operation. For sub-components,
   insert and read back the three names under `TestComponent` before selecting
   the before name for the disposable bug.
2. Split bzr probing by field shape: feed stdin from a jq-built
   `{"rh_sub_components":{"TestComponent":["<subcomponent>"]}}` object to
   `--field-json -`, retain scalar `--field` for the remaining fields, and
   assert response `sub_components`.
3. Supply distinct bzr values and assert their REST and bzr view readbacks;
   make whiteboard bzr updates and filters cover each named whiteboard.

**Acceptance:** no successful bzr no-op can satisfy a phase ID; each target
release write uses a SQL-verified configured value; an
expected gap is emitted only after the structured sub-component path has been
tried.

## Task 2 — model transitions and the no-op control

**Files:** modify `tests/functional/pybz/container-tests.sh`.

**Interfaces:** consume Task 1's exact command shapes and field/value sequence;
provide fake state/readback behavior to the RHBZ phase.

**Verification.**

- Contract: modeled python and bzr writes have independent persisted state.
  Mode: focused-test. Red: a modeled bzr no-op yields four phase failures.
  Green: `bash tests/functional/pybz/container-tests.sh` exits 0.
- Contract: fixture observes `--field-json -` and all whiteboard fields.
  Mode: focused-test. Red: malformed/missing structured input or omitted
  internal/QA update fails. Green: same command exits 0.

**Steps.**

1. Store separate before, python, and bzr values for each fixture bug and make
   curl/read-bzr output the current state rather than only the python values.
2. Parse stdin JSON for the `rh_sub_components` object (reject a `component`
   object or scalar input) and scalar fields for the other updates; record all
   three whiteboards.
3. Add a successful no-op toggle that returns exit zero without state mutation,
   then assert four failures, zero passes, and zero gaps.

**Acceptance:** the fixture turns the exact false-positive failure mode into a
red control without depending on live RHBZ availability.

## Task 3 — reconcile published parity evidence

**Files:** modify `docs/dev/python-bugzilla-parity.md`.

**Interfaces:** consume Task 1's four IDs and executed command surfaces.

**Verification.**

- Contract: each of four RHBZ field rows points at its stable phase ID and uses
  the actual bzr invocation. Mode: focused-test. Red: existing report fixture
  rejects stale rows. Green: `bash tests/functional/pybz/container-tests.sh`
  exits 0.

**Steps.** Replace the predicted-gap descriptions with the structured
sub-component command and verified scalar field commands, retaining the stable
test IDs.

**Acceptance:** the published matrix describes the proof the live phase runs.

## Final verification and cleanup

Run `make lint`, `make test`, `make release`, then `make functional-compare-rhbz`.
Expect zero exits; the live summary includes the four phase IDs and no failures.
The runner removes its temporary exchange directory and RHBZ container state.
