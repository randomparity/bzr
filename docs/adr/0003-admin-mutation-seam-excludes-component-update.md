# 0003 — Admin create/update seam is linear-only and excludes component update

- Status: Accepted
- Date: 2026-06-23
- Issue: #420

> **Superseded by [0037](0037-remove-unsupported-component-update.md)**
> (2026-09-02)

## Context

The admin resource commands (`product`, `component`, `group`, `user`) each
implement `create` and `update` handlers that repeat an identical five-step
control-flow skeleton: read the output format, branch on dry-run to emit a
`DryRunResult`, connect, call one resource API method, and write an
`ActionResult`. Desloppify flags this as duplication across the create/update
paths.

Seven handlers follow this skeleton exactly (`product` create/update, `group`
create/update, `user` create/update, `component/create`). One does not:
`component/update` accepts two target forms — a numeric component ID or a
`--product`/`--component` name pair — which makes it diverge in two ways:

- its dry-run branch serializes one of two different payload shapes depending on
  the target form, not a single params object; and
- it performs an extra `get_product` round-trip between connect and the API call
  to resolve a name pair to a numeric ID.

Issue #420 explicitly requires that the extraction "not introduce a broad
generic command framework" and that it "preserve resource-specific validation
clarity."

## Decision

Introduce one narrow seam, `crate::commands::runtime::mutation::run`, that owns
**only** the linear create-shaped / name-keyed mutation flow:
dry-run-preview-or-(connect, execute, write). It takes an owned `DryRunPreview`
(resource kind, serializable params, human message) and a single `execute`
closure that consumes the client plus params and returns a `Committed` result.

The seven linear handlers migrate onto it. `component/update` stays bespoke and
is explicitly out of the seam's scope. The seam hard-codes an empty `ids` slice
for the dry-run preview, which is correct for every linear handler (all are
create-shaped or name-keyed) and documents that id-keyed previews are not its
job.

## Consequences

- The five-step skeleton exists once instead of seven times; each handler keeps
  only its resource-specific param building, validation, messages, API call, and
  result construction.
- `component/update` remains the single place that understands dual-target
  resolution, so that complexity is not smeared into a shared abstraction.
- The seam cannot serve id-keyed dry-run previews without being widened; that is
  a deliberate boundary, recorded here so a future id-keyed mutation is added as
  a sibling path rather than by overloading `run`.
- Output, payloads, and error behavior for the migrated handlers are unchanged;
  the existing per-resource test suites are the regression guard.

## Considered & rejected

- **A generic `Mutation` trait implemented per resource.** Rejected: it is the
  prohibited generic command framework and would hide the resource-specific
  validation #420 says to preserve.
- **Bending the seam to also cover `component/update`** via dry-run-render and
  pre-call-resolve closures. Rejected: it adds framework surface for exactly one
  caller and makes the common path harder to read.
- **Extracting only the dry-run branch.** Rejected: it leaves half the
  duplicated skeleton (connect + commit + write) in place.
