# RHBZ field transition proof design

## Authority

Issue #795, frozen by its `WORK:SCOPE` token `q795-5c8a4e1d`, requires the
RHBZ field comparison to prove distinct persisted python-bugzilla and bzr
updates. It excludes the anonymous/auth audit (#663) and unrelated RHBZ
extensions (#774, #796).

## Problem

The phase currently reads the python-bugzilla value after that arm and gives the
same value to bzr. A successful bzr no-op therefore passes. The whiteboard arm
also verifies only devel. Its sub-component probe treats the scalar `--field`
shape as a capability gap without first trying bzr's supported JSON object
input.

## Design

Each of the four stable phase IDs keeps one disposable bug. Its values form a
three-state sequence: fixture before value, python-bugzilla reference value,
and a distinct bzr value. The phase reads each state from RHBZ before advancing.
For target release, the SQL fixture provisions and reads back the three distinct
run-token release rows before the bug is created, so each transition writes a
valid configured value rather than reusing a single release.
The sub-component bzr arm supplies
`{"rh_sub_components":{"TestComponent":["<subcomponent>"]}}` through
`--field-json -`, then reads response field `sub_components`; the source write
field is therefore explicitly `rh_sub_components`. Target release and fixed-in retain their
existing `--field` bzr path. The whiteboard bzr arm updates and reads devel,
internal, and QA values together.

The shell self-test models these transitions separately and reruns the phase
with a successful bzr no-op. That control must produce four failures rather
than a parity pass or expected gap. It also verifies that the structured
sub-component input reaches bzr and that all three bzr whiteboard values are
observable. The parity matrix changes the four rows to documented observed
capabilities rather than anticipated gaps.

No ownership transition is needed: the RHBZ phase owns field comparison state,
the self-test owns fixture behavior, and the parity matrix owns the published
classification.

## Failure model

- **Actors and deployments:** local developers and CI run the disposable RHBZ
  comparison container; production RHBZ is outside this design.
- **Invariants and assets at stake:** each accepted result must prove a named
  bzr mutation persisted after a distinct reference mutation; temporary
  exchange captures stay private to the run.
- **Accepted failure classes:** unavailable RHBZ extensions or permissions are
  positive-control failures because the existing phase already rejects them;
  bzr rejection after structured input remains an observed gap only with its
  exact diagnostic.
- **Covered elsewhere:** anonymous/auth behavior is #663; ExternalBugs and
  component-extension work are #774; multi-version extension semantics are
  #796.

## Success

- The four named RHBZ phase IDs each prove before, reference, and bzr values by
  real-server readback; whiteboards means the bounded set devel/internal/QA.
- The sub-component arm uses `--field-json` with the supported
  `rh_sub_components` object shape and reads `sub_components`.
- The self-test's successful bzr no-op causes all four phase IDs to fail.
- The RHBZ parity matrix accurately identifies the four executed bzr surfaces.

## Validation

- `bash tests/functional/pybz/container-tests.sh` proves normal, missing
  controls, known diagnostic, unexpected failure, and successful no-op paths.
- `make lint` and `make test` protect repository shell and Rust guardrails.
- `make release` followed by `make functional-compare-rhbz` is the decisive
  live proof against the RHBZ container.
