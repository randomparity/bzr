# CLI and parity reconciliation design

## Problem

The published command tree and embedded agent reference omit shipped commands,
while parity prose retains classifications contradicted by current RHBZ evidence.
The parity fixture only pins literal rows, so it cannot catch a misleading
command-to-capability claim.

## Scope

Update the command tree, detailed/embedded command references, parity matrix,
and its existing fixture. This documents current behavior only; it adds no CLI
option, command, or documentation framework.

### Failure model

- A row may claim an unsupported command or flag: fixture checks source-backed
  command/flag mappings.
- A row may lose its evidence ID: fixture checks the bounded required IDs.
- A stale RHBZ classification may return: exact capability rows stay pinned.
- Out-of-scope behavior remains unchanged and is not asserted here.

## Success

The tree exposes `config import-bugzillarc`; both references describe shipped
auth and import behavior plus match-type flags. The report labels proven RHBZ
writes as parity, keeps missing reads and unresolved work explicit, and names
deliberate non-goals accurately. The self-test rejects the bounded stale
contradictions above.

## Validation

- `focused-test`: `bash tests/functional/pybz/container-tests.sh --self-test`
  validates the documentation fixture and its controlled failures.
- `focused-test`: `make skills-test` regenerates and validates the embedded
  agent skill payload after its source reference changes.
- `focused-test`: `make lint` and `make test` validate repository guardrails.
