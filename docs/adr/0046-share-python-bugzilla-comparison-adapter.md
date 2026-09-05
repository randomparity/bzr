# ADR 0046: Share one fixed python-bugzilla comparison adapter

## Status

> **Superseded by [0051](0051-share-adapter-with-bounded-local-proofs.md)** (2026-09-05)

## Context

ADR 0044 established a fixed JSON adapter inside the python-bugzilla sidecar for comparisons that
need library methods rather than CLI commands. The first adapter is named `bug-lifecycle.py`, but
issue #668 needs the same authenticated connection, transport observation, private request files,
and safe JSON result boundary for comments, attachments, users, groups, products, and components.
Adding a resource-specific adapter would duplicate that boundary and give the runner multiple
commands with overlapping security and serialization contracts.

## Decision

Rename the existing helper to `python-bugzilla-adapter.py` and keep one fixed dispatch table for
all python-bugzilla library comparisons. Existing lifecycle operations retain their request and
transport behavior. New operations accept an explicit `REST` or `XMLRPC` transport only where a
test must exercise both transports; otherwise they use python-bugzilla's normal probe.

The Red Hat-only component-update comparison is the one local-proof operation. The adapter
constructs the pinned library's `Bugzilla` object without a URL, injects a recorder implementing
only the backend's `component_update` call, and invokes the public `editcomponent` method. Its
output carries a null transport and the normalized recorded request. This proves the pinned
client surface without contacting a server or representing the result as transport evidence.

The runner stages the helper once at `/work/compare/python-bugzilla-adapter.py`, and
`run_pybz_adapter` remains the only shell entry point. JSON inputs, outputs, and attachment files
stay under the private `/work/compare` exchange directory. Attachment input must resolve to a
regular, non-symlinked private file there; paths, API keys, and upstream exception text are not
echoed on failure.

Resource comparison phases share shell mechanics for invocation, capture, transport validation,
and canonical JSON comparison, but each phase owns its fixtures and semantic projections. A phase
compares persisted state, not response or terminal presentation bytes.

## Consequences

- One tested adapter boundary serves every python-bugzilla library comparison.
- Renaming the helper updates the lifecycle runner and its focused fixtures in the same change;
  no compatibility alias remains on this internal test surface.
- The dispatch table grows, while each handler remains a small resource-specific operation.
- Explicit transport selection is test evidence, not a production capability or public CLI.
- A null transport is valid only for the fixed component-update local proof; network operations
  still fail when observed transport is absent or unknown.
- Attachment exchange remains confined to files the comparison runner created privately.

## Considered & rejected

- **Add one adapter per resource family.** judgment: this duplicates path validation, credential
  loading, transport normalization, serialization, and safe error handling across test helpers.
- **Drive every operation through the python-bugzilla CLI.** verified: python-bugzilla 3.3.0
  `bugzilla/base.py` exposes `get_comments`, `updateattachmentflags`, `createuser`, `updateperms`,
  `getgroup(s)`, `product_get`, and `addcomponent` as library methods without matching CLI actions.
- **Keep the lifecycle-specific filename after widening the dispatch table.** judgment: the name
  would describe only one consumer and become false guidance for later comparison phases.
- **Move comparison mechanics into production Rust.** judgment: no compiled product behavior is
  required to compare the two clients, and a test-only production surface would widen the product.
