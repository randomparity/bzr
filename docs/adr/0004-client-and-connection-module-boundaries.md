# 0004 — Client and connection orchestration module boundaries

- Status: Accepted
- Date: 2026-06-23
- Issue: #422

## Context

Two modules bundle several orchestration responsibilities behind a single file
and a single `impl` block, and desloppify flags both as large/complexity
hotspots:

- `src/client/mod.rs` (789 lines) mixes, in one `impl BugzillaClient`: client
  construction, request building, auth application, the send/retry state
  machine, the 401 alternate-auth fallback, response-body parsing, Bugzilla
  HTTP-200 error classification, multi-envelope tolerance, and the thin per-verb
  helpers.
- `src/commands/runtime/shared/connection.rs` (609 lines) mixes:
  connection-target resolution (inline vs. named server), the `ConnectContext`
  value object and its persistence helpers, TLS trust handling (TOFU, pin
  rotation, issuer change, cached-connection probe), and settings detection +
  persistence.

Neither file has a single axis of change, so unrelated edits collide and the
test siblings have grown large. The change must preserve all observable behavior
and add no new public API surface (issue #422).

## Decision

Split each module by relocation into cohesive submodules named for the single
concern they own. No behavior changes, no new generic abstraction, no
compatibility shim.

`src/client/` becomes a directory module:

- `mod.rs` — `BugzillaClient`/`BugzillaClientConfig` definitions, `new()`
  construction, `dispatch_xmlrpc_first`, shared DTOs, re-exports.
- `transport.rs` — send/retry state machine, auth application, 401 alternate-auth
  fallback.
- `response.rs` — body parsing, Bugzilla-200 error classification, envelope
  tolerance, redacted body previews.
- `request.rs` — the per-verb helpers (`get_json`, `post_json_id`, …) that
  compose transport + response.

`src/commands/runtime/shared/connection/` becomes a directory module:

- `mod.rs` — the `connect_and_configure` orchestrator and credential check.
- `target.rs` — `ConnectContext`, `ConnectTarget`, `resolve_connect_target`,
  `extract_hostname`.
- `tls_trust.rs` — TOFU, pin rotation, issuer change, TLS probing.
- `detect.rs` — settings detection + persistence glue.

This works as a pure move because Rust privacy is module-scoped: submodules read
the parent type's private fields and free items without new accessors, and an
`impl` block may be spread across the module tree. Visibility widens only to
`pub(super)` *within* each module tree where a sibling now calls a previously
private item; nothing crate-external changes. Tests relocate to per-leaf
`*_tests.rs` siblings matching where the code now lives, satisfying
`make check-test-layout`; every existing test function is preserved.

## Consequences

- Each concern (retry/backoff, envelope tolerance, TOFU, target resolution, …)
  lives in one file with one reason to change; the oversized files and their
  oversized test siblings are gone.
- A small amount of internal visibility widens from private to `pub(super)`
  within the `client` and `connection` trees. This is recorded as a deliberate
  boundary: the widening stops at the module tree and must never reach
  `pub(crate)`/`pub` for these items.
- Public signatures (`BugzillaClient`, `BugzillaClientConfig`,
  `connect_and_configure`, `detect_server_settings*`, `DetectedServerSettings`)
  and all observable behavior are unchanged; the existing test suite is the
  regression guard, and the pre/post test-name inventory is diffed to prove no
  test was dropped.
- `dispatch_xmlrpc_first` and `new()` stay in `client/mod.rs` with the type
  definition rather than moving to a fourth submodule; they are central and
  small, and a further split would cost readability without reducing any hotspot.

## Considered & rejected

- **In-file reorganization only** (labelled `impl` blocks / helper structs, no
  new files). Rejected: the files stay large, desloppify still flags them, and it
  does not deliver the clear ownership boundaries the issue asks for.
- **Keep tests in the existing `mod_tests.rs` and reach moved private fns via
  `#[cfg(test)]` re-exports.** Rejected: violates the per-leaf sibling convention
  enforced by `make check-test-layout` and defeats the SonarCloud CPD exclusion's
  intent to spread test boilerplate across siblings.
- **Introduce a generic request-pipeline trait across the verb helpers.**
  Rejected: premature abstraction the issue prohibits; the helpers are already
  thin concrete methods.
