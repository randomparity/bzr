# ADR 0075 — Catalogue RHBZ custom fields with real-server controls

## Status

Accepted (2026-09-12)

## Context

Issue #775 needs live RHBZ evidence for sub-components, target release,
fixed-in, and the devel, internal, and QA whiteboards. Stock images do not
provide those extensions. The existing generic-field comparison proves neither
the RHBZ field names nor the permissions and persisted response shapes.

## Decision

Add a RHBZ-only `09-rhbz-fields` phase after smoke and before the ExternalBugs
catalogue. The phase will create disposable bugs in the seeded component,
validate the needed administrator permissions and field metadata, and use the
python-bugzilla adapter's Red Hat argument names to make each mutation. It
will read the live server state after each positive control, then probe the
corresponding `bzr bug create` or `bzr bug update --field` surface and classify
the observed result. The four stable IDs are `sub-components`,
`target-release`, `fixed-in`, and `whiteboards`.

## Consequences

The evidence is isolated to the RHBZ runner and cannot change stock comparison
coverage. Phase 09 runs while the seeded component remains active; phase 08's
component-deactivation check remains last. A missing field, changed permission,
rejected python-bugzilla call, or unexpected bzr outcome fails rather than
producing a parity claim. The whiteboards remain one test because their shared
fixture and update operation are one server contract.

## Considered & rejected

- **Use generic arbitrary-field writes.** verified: python-bugzilla's
  `_rhconverters.py` maps these public arguments to RHBZ wire names, so the
  existing generic helper would not prove the documented client surface.
- **Add the proof to a stock phase.** verified: issue #775 states the fields
  are unavailable in stock functional images; ADR 0073 isolates RHBZ runs.
- **Implement bzr support while measuring it.** verified: issue #775 excludes
  bzr feature implementation; judgment: catalogue evidence should not widen
  into product work.
