# Split Client Connection and Auth Orchestration — Design

**Date:** 2026-06-23
**Branch:** `feat/split-client-connection-orchestration-422`
**Issue:** #422 — Split client connection and auth orchestration
**ADR:** [0004](../adr/0004-client-and-connection-module-boundaries.md)

## Background

Two modules have accreted multiple orchestration responsibilities and are
flagged by desloppify as large/complexity hotspots:

- `src/client/mod.rs` (789 lines) — the `BugzillaClient` HTTP layer. One `impl
  BugzillaClient` block mixes client construction, request building + auth
  application, the send/retry state machine, the 401 alternate-auth fallback,
  response parsing, Bugzilla-specific error classification, envelope tolerance,
  and the thin per-verb helpers (`get_json`, `post_json_id`, …).
- `src/commands/runtime/shared/connection.rs` (609 lines) — the
  `connect_and_configure` orchestrator. One file mixes connection-target
  resolution (inline vs. named server), the `ConnectContext` value object and
  its persistence helpers, TLS trust handling (TOFU prompt, pin rotation, issuer
  change, cached-connection probe), and settings detection + persistence.

Neither module has a single axis of change: a tweak to retry backoff, to the
TOFU prompt, or to envelope tolerance all land in the same oversized file, and
the test siblings (`mod_tests.rs`) have grown correspondingly large.

## Goals (success criteria)

- The two responsibilities-bundles are split into cohesive submodules with clear
  ownership, each named for the single concern it owns (see Design).
- **No observable behavior change.** Public command behavior, auth-fallback
  behavior (401 header↔query retry), API-mode detection, TLS/TOFU/pin-rotation
  behavior, retry/backoff behavior, and connection-target resolution are
  byte-for-byte identical. This is verified by the existing test suites passing
  **without weakening, deleting, or skipping any assertion**, and by the full
  `cargo test` suite plus `cargo clippy --all-targets --all-features -- -D
  warnings` staying green.
- **No new public API surface.** The crate-external signatures
  (`BugzillaClient`, `BugzillaClientConfig`, `connect_and_configure`,
  `detect_server_settings*`, `DetectedServerSettings`) are unchanged. Internal
  visibility may widen from `pub(super)`/private to `pub(super)`/`pub(crate)`
  *within* the `client` / `connection` module trees only as far as the split
  mechanically requires — never beyond the module that already owns the symbol.
- **No compatibility shims or dual paths.** The old flat files are replaced, not
  deprecated. No re-export facade that duplicates the public surface, no
  parallel old/new code path.
- Tests are relocated to per-leaf `*_tests.rs` siblings matching where the code
  now lives, satisfying `make check-test-layout`. Every existing test function
  is preserved (moved, not rewritten); the post-split test-name set is a
  superset-by-relocation of the pre-split set (same names, new files).
- `make lint` (fmt + clippy + check-test-layout + check-no-spawn) and `cargo
  test` are green.

## Design

### Why module privacy makes this a pure move

Rust privacy is module-scoped. Submodules of `client` can read the private
fields of `BugzillaClient` (`auth`, `api_key`, `email_hint`, `retry_max`,
`http`, `base_url`, `api_mode`, `xmlrpc`) declared in `client/mod.rs`, and an
`impl BugzillaClient { … }` block may be spread across sibling files in the same
module tree. So moving methods into submodules requires **no** new field
accessors and **no** struct change — only that fields the moved methods touch be
visible to the child module (already true: same module tree). The same holds for
the free functions and `pub(super)` items in `connection.rs`.

### `src/client/` split

`mod.rs` retains: the module declarations + public re-exports, the
`BugzillaClient` / `BugzillaClientConfig` / `PreparedAuth` definitions, response
DTOs that are shared across submodules (`UserSearchResponse`, `IdResponse`,
`UserDetailLevel`), `encode_path`, `new()` (construction), `set_retry_max`,
`url()`, `xmlrpc_client()`, and `dispatch_xmlrpc_first` (API-mode dispatch — a
small, central concern that belongs with the type).

New submodules (each an additional `impl BugzillaClient` block + its free
helpers):

| Submodule | Owns | Moved items |
|---|---|---|
| `client/transport.rs` | The send/retry state machine and auth-on-the-wire | `send`, `send_raw`, `is_transient`, `sleep_before_retry`, `retry_with_alternate_auth`, `apply_auth`, `apply_alternate_auth`, `safe_url`, free `strip_auth_query_param` |
| `client/response.rs` | Body parsing, Bugzilla error classification, envelope tolerance, body-preview redaction | `parse_json`, `parse_json_value`, `parse_body_to_value`, `check_mutation_response`, `check_bugzilla_200_error`, `check_response_status`, `try_envelopes`, `has_data_fields`, free `deserialize_code`, `format_body_preview`; consts `DATA_KEYS`, `BODY_PREVIEW_MAX_BYTES`, `BODY_TRACE_MAX_BYTES`; types `ErrorResponse`, `EnvelopeCandidate` |
| `client/request.rs` | The thin per-verb helpers that compose transport + response | `get_json`, `get_json_query`, `get_json_value`, `post_json_id`, `put_json`, `put_json_response` |

`apply_auth` lives in `transport.rs` (it is called only by the verb helpers and
the send path; placing it with the wire layer keeps auth-application next to
auth-fallback). `request.rs` calls `self.apply_auth(...)`, `self.send(...)`,
`self.parse_json(...)` — all cross-submodule method calls on the same type,
which compile because they are inherent methods on `BugzillaClient`.

### `src/commands/runtime/shared/connection/` split

Convert the file module into a directory module. `connection/mod.rs` retains:
the top-level `connect_and_configure` orchestrator, `require_credentials_for_connection`,
and the `pub(super)`/`pub(crate)` re-exports the parent `shared` module and the
test siblings consume.

New submodules:

| Submodule | Owns | Moved items |
|---|---|---|
| `connection/target.rs` | Resolving the connection target and the `ConnectContext` value object | `ConnectContext` (+ its inherent methods `email_hint`, `persist_settings`, `persist_locked`, `hostname`, `build_client`), `ConnectTarget`, `resolve_connect_target`, free `extract_hostname` |
| `connection/tls_trust.rs` | TLS trust decisions: TOFU, pin rotation, issuer change, probing | `should_offer_tofu`, `tls_uses_default_trust`, `probe_tls`, `handle_tofu`, `handle_pin_rotation`, `classify_and_handle_tls_failure`, `probe_cached_connection`, `pin_current_cert_for_session` |
| `connection/detect.rs` | Settings detection + persistence glue | `persist_detected_settings`, `detect_settings`, `detect_and_build_client`, `detect_with_tofu_fallback`, `DetectOrClient` |

Cross-submodule calls (e.g. `tls_trust::handle_tofu` calls
`detect::detect_and_build_client`; `detect` calls `ConnectContext` methods;
`connect_and_configure` calls all three) are resolved by `pub(super)`
visibility within the `connection` tree plus `use` imports. `ConnectContext`'s
inherent methods that submodules call (`build_client`, `persist_settings`,
`persist_locked`, `hostname`, `email_hint`) widen from private to `pub(super)`
so sibling submodules can call them; the struct fields it exposes to submodules
likewise widen to `pub(super)`. None of this widens crate-external surface.

### Test relocation

Per the repo test-layout rule (sibling `*_tests.rs`, no inline `mod tests`),
each new source leaf that carries a `#[cfg(test)] #[path = "<leaf>_tests.rs"]
mod tests;` gets its own sibling file. Tests move to the sibling matching the
code they exercise:

- `client/mod_tests.rs` keeps tests of construction / `dispatch_xmlrpc_first` /
  `url`. Tests of `format_body_preview`, `try_envelopes`, `parse_body_to_value`,
  `check_response_status`, `check_bugzilla_200_error` move to
  `client/response_tests.rs`. Tests of `apply_alternate_auth` / send / retry move
  to `client/transport_tests.rs`. Verb-helper tests (if any) move to
  `client/request_tests.rs`.
- `shared/mod_tests.rs` keeps tests that exercise `connect_and_configure` and
  body-source helpers. Connection-internal tests move to the sibling matching
  the submodule: `extract_hostname`/`ConnectContext` → `connection/target_tests.rs`;
  `should_offer_tofu`/`tls_uses_default_trust`/`probe_tls`/`classify_and_handle_tls_failure`
  → `connection/tls_trust_tests.rs`; `detect_with_tofu_fallback`/`persist_detected_settings`
  → `connection/detect_tests.rs`.

The `#[cfg(test)]` re-export lists in `client/mod.rs` and
`shared/mod.rs`/`connection/mod.rs` are updated so each relocated test still
reaches its target symbols. A test that drives a now-cross-module private symbol
imports it from the submodule (`use super::tls_trust::should_offer_tofu;` or via
a `pub(super)` re-export in `connection/mod.rs`), never by re-widening the symbol
to `pub(crate)`.

Each relocated sibling starts with the same file-level inner attribute the
origin file used (`#![expect(clippy::unwrap_used)]` etc.) only if its moved tests
actually trip that lint; siblings whose tests don't trip it omit it (per the
existing convention).

### Shared test helpers must be promoted before tests move

The origin test siblings define local helper fns that are shared across tests
which this split routes into *different* destination siblings, so they cannot
travel with any single relocated test:

- `client/mod_tests.rs` defines `debug_logging_guard`,
  `multibyte_body_crossing_preview_boundary`, `has_no_auth_header`,
  `has_no_auth_query_param`, and `bug_ok_body`, used by tests headed for both
  `response_tests.rs` and `transport_tests.rs`. (It already imports
  `test_helpers::{test_client, test_client_query_param}` from the existing
  `client/test_helpers.rs`.)
- `shared/mod_tests.rs` defines `load_config`, `ctx_at`, `connect_context`,
  `write_config`, `write_credentialless_config`, and `mount_detection_mocks`,
  used by tests headed for `connection/target_tests.rs`,
  `connection/tls_trust_tests.rs`, and `connection/detect_tests.rs`.

**Decision:** before any test is relocated, promote each cross-sibling helper
into a shared test-helpers module reachable from every destination sibling:

- For the client tree, extend the existing `src/client/test_helpers.rs` (already
  `#[cfg(test)] pub(super) mod test_helpers;`) with the client-side helpers, and
  have each relocated sibling `use super::test_helpers::…`.
- For the connection tree, add `src/commands/runtime/shared/connection/test_helpers.rs`
  as `#[cfg(test)] pub(super) mod test_helpers;`, move the six shared connection
  helpers into it at `pub(super)` visibility, and have each relocated sibling
  `use super::test_helpers::…`.

A helper used by tests that all land in a single destination sibling stays a
private fn in that sibling — only genuinely cross-sibling helpers are promoted, so
the shared module does not become a dumping ground. This is a prerequisite step,
sequenced ahead of test relocation in the plan.

### Verification that nothing was lost

Two checks, run together:

1. **No test dropped or renamed.** The pre-split inventory of test-function names
   across the origin test files (115 functions) is captured before the move; after
   relocation the union of test-function names across all post-split siblings must
   equal that set.
2. **No test silently weakened.** Name-equality alone does not prove a moved test
   kept its assertions, so the move is verified with `git diff -M` rename
   detection: each relocated test must show as a pure move (zero content delta) of
   its body. Any non-zero delta on a moved test body must be a deliberate,
   reviewed change (e.g. an import-path adjustment), not an assertion change.

`cargo test --lib` reports the same passing count (2041) before and after, and
`make lint` (including `check-test-layout`) stays green.

## Non-goals

- No change to any public (crate-external) signature or behavior.
- No change to the auth-detection probe logic itself (`client/auth/` is already
  split and is out of scope except as a call target).
- No change to retry/backoff math, TLS classification, or persistence semantics
  — only the file each function lives in changes.
- No new generic abstraction, trait, or indirection introduced to "unify" the
  pieces; the split is by relocation into cohesive files, not by adding layers.
- No re-export facade in `client/mod.rs` beyond what already exists / what the
  parent + tests require.

## Risks

- **Accidental behavior change during the move.** Mitigated: the move is
  mechanical (cut/paste of whole functions), the full existing test suite is the
  characterization net and must pass unchanged, and the test-name inventory is
  diffed before/after.
- **Visibility churn introducing wider-than-needed `pub`.** Mitigated: widen only
  to `pub(super)` within the owning module tree; a clippy/ty pass and manual
  review confirm no `pub(crate)`/`pub` leaks beyond the tree.
- **`check-test-layout` / `check-no-spawn` regressions** from the new files.
  Mitigated: run `make lint` (which includes both) before each commit.
- **Mutation-test skip attributes** on `handle_tofu`/`handle_pin_rotation` must
  travel with the functions to `tls_trust.rs` intact.

## Considered & rejected

- **In-file reorganization only** (group methods into labelled impl blocks /
  helper structs without new files). Rejected: the files stay large and desloppify
  would still flag them; it does not deliver the "clear ownership boundaries" the
  issue asks for.
- **Keep all tests in the existing `mod_tests.rs` and reach moved private fns via
  `#[cfg(test)]` re-exports.** Rejected: violates the per-leaf sibling convention
  enforced by `make check-test-layout`, and concentrates test boilerplate the
  SonarCloud CPD exclusion is meant to spread across siblings.
- **Extract a shared generic "request pipeline" trait across client verbs.**
  Rejected: premature abstraction; the verb helpers are already thin and the
  issue prohibits broad frameworks. The split keeps them as concrete methods.
- **Move `dispatch_xmlrpc_first` and `new()` out of `mod.rs`.** Rejected:
  construction and API-mode dispatch are central to the type's identity and are
  small; keeping them with the struct definition aids readability more than a
  further split would.
