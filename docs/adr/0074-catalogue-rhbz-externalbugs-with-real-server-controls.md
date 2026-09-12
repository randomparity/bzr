# ADR 0074 — Catalogue RHBZ ExternalBugs with real-server controls

## Status

Accepted (2026-09-12)

## Context

Issue #774 needs evidence for three `ExternalBugs` mutations and
`Component.update` on the isolated RHBZ server from ADR 0073. The stock
component comparison only records python-bugzilla's local request shape, so it
cannot establish that RHBZ accepts the operation or its privileges and fixture.
`bzr` deliberately has no equivalent component-update or ExternalBugs command.

## Decision

Append one RHBZ-only phase after the extension smoke phase. Add narrowly
validated python-bugzilla adapter operations for `add_external_tracker`,
`update_external_tracker`, `remove_external_tracker`, and `editcomponent`.
The phase creates its own product, component, and bug; configures the minimum
external tracker fixture; proves add, update, removal, and component mutation
by reading RHBZ state; then records the absent `bzr` surfaces as controlled
expected gaps for #774. Replace the former local-only component-update parity
row with the four real-server rows and stable phase IDs.

## Consequences

The catalogue runs only with `BZR_BZ_VERSION=rhbz`, so stock comparison phases
and their evidence remain unchanged. A changed RHBZ extension schema or
privilege model produces a functional failure instead of silently preserving a
local request-shape claim. The adapter remains the sole python-bugzilla call
boundary, while the phase owns server fixture setup and persisted-state checks.

## Considered & rejected

- **Keep the local component-update recorder.** verified: `tests/functional/
  compare/05-products-components.sh` exercises `component_update_shape` with
  `LOCAL` transport, so it cannot establish real-server acceptance.
- **Extend the stock component phase.** verified: ADR 0073 isolates RHBZ
  catalogue children after its smoke phase and keeps stock runners unchanged.
- **Add bzr commands as part of the proof.** verified: issue #774 explicitly
  excludes implementing bzr features; judgment: controlled expected gaps are
  the smallest truthful classification for absent public surfaces.
