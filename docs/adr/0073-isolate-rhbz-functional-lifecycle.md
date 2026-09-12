# ADR 0073 — Isolate the RHBZ functional lifecycle from stock versions

## Status

Accepted (2026-09-12)

## Context

Issue #675 needs real-server evidence for Red Hat Bugzilla extensions. The stock
Bugzilla 5.0, 5.2, and 5.3 images cannot serve those extensions, and the public
RHBZ fork has its own source tree and schema. The RHBZ image must not make the
ordinary stock functional or comparison matrices slower or less reproducible.

## Decision

Add `rhbz` as a separately selected functional version. Its Containerfile clones
the public RHBZ repository at the issue-pinned full revision and provisions that
checkout's disposable database. Reuse the existing version-derived container name,
port lookup, start, stop, and readiness lifecycle. Add a dedicated RHBZ comparison
runner and smoke phase; it verifies the reachable server advertises ExternalBugs,
SubComponents, and RedHat before future catalogue phases can use it. Expose this
route through its own Make target. Do not include `rhbz` in stock all-version arrays.

## Consequences

The RHBZ source and database are isolated to one disposable container, while the
existing lifecycle retains one owner for cleanup and runtime-selected ports. A
functional run downloads and builds a larger external source tree only when the
explicit RHBZ target is selected. Catalogue children can append RHBZ phases after
the smoke contract without changing stock comparison phases.

## Considered & rejected

- **Copy extension directories into the bz50 image.** verified: issue #675 states
  that RHBZ is a modified server and schema, not extensions transferable to stock
  Bugzilla.
- **Use the Red Hat-shaped response proxy as server proof.** verified: the #665
  RHBZ addendum explicitly excludes proxy evidence as proof of RHBZ behavior.
- **Add RHBZ to `functional-test-all` and `functional-compare-all`.** judgment:
  an opt-in prerequisite should not alter the stock matrix or its routine cost.
