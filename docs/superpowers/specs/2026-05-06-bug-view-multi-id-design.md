# `bzr bug view`: accept multiple bug IDs and `--permissive` flag

**Date:** 2026-05-06
**Issue:** [#156](https://github.com/randomparity/bzr/issues/156) (`bzl-parity`, `enhancement`)
**Parity context:** [`docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md`](2026-05-06-bzl-parity-review-design.md), Issue A
**Reference impl:** `reference/bzl/bzl-get` (`Bug.get` with `{ids, permissive}`)

## Goal

Bring `bzr bug view` to parity with `bzl-get`:

- Accept one or more bug IDs as positional arguments (variadic).
- Accept a `--permissive` flag that suppresses per-bug access errors,
  surfacing them as inline error blocks instead of a non-zero exit.
- Single-ID invocation behavior — both table and JSON output — is
  unchanged.

## Non-goals

- No new client method. Multi-ID fetch is a sequential client-side loop
  over the existing `client.get_bug()`.
- No parallel/concurrent fetch.
- No reuse of the server-side `GET /rest/bug?id=…` multi-ID endpoint
  (it silently drops inaccessible IDs and would lose the per-ID error
  detail `--permissive` is supposed to surface).
- No new `BzrError` variants. `BatchPartialFailure` (exit 11) is NOT
  used by `view`; failed reads change no server state.

## CLI surface

In `src/cli/bug.rs`, `BugAction::View` becomes:

```rust
View {
    /// Bug ID(s) or alias(es). Aliases and numeric IDs may be mixed.
    #[arg(required = true, num_args = 1..)]
    ids: Vec<String>,
    /// On batch view, return inline error rows for inaccessible bugs
    /// instead of bailing on the first failure (exit 0 even if some fail).
    #[arg(long)]
    permissive: bool,
    /// Only return these fields (comma-separated)
    #[arg(long)]
    fields: Option<String>,
    /// Exclude these fields (comma-separated)
    #[arg(long)]
    exclude_fields: Option<String>,
},
```

Help-text examples added to the existing doc-comment block:

```text
bzr bug view 12345
bzr bug view 12345 12346 12347
bzr bug view 12345 my-alias 12347 --permissive
bzr bug view 12345 --json | jq .summary           # single-ID JSON unchanged
```

`--permissive` with exactly one ID is rejected at handler entry with
`BzrError::InputValidation` ("`--permissive` only meaningful with
multiple IDs") → exit 7. This catches the likely user error of treating
`--permissive` as "tolerate any view failure."

## Dispatch / handler

`src/commands/bug/view.rs` branches single-vs-multi, mirroring
`bug update`'s structure:

```rust
pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::View { ids, permissive, fields, exclude_fields } = action
    else { unreachable!() };

    if *permissive && ids.len() == 1 {
        return Err(BzrError::InputValidation(
            "--permissive only meaningful with multiple IDs".into(),
        ));
    }

    let inc = fields.as_deref();
    let exc = exclude_fields.as_deref();

    if ids.len() == 1 {
        view_single(client, &ids[0], inc, exc, format).await
    } else if *permissive {
        view_batch_permissive(client, ids, inc, exc, format).await
    } else {
        view_batch_strict(client, ids, inc, exc, format).await
    }
}
```

Three private helpers, each one job:

- **`view_single`** — current behavior verbatim. `client.get_bug()` then
  `output::print_bug_detail()`. Single-ID JSON shape preserved (a plain
  `Bug` object).
- **`view_batch_strict`** — sequential `for id in ids { client.get_bug(...).await? }`.
  Output mode determines whether successes are eager-printed:
  - **Table**: print each detail block + divider as it arrives. The
    first `Err` propagates; preceding successes have already been
    written to stdout.
  - **JSON**: collect all `Bug`s in a buffer; on any error, return
    `Err` with an empty stdout. Eager-print would emit
    concatenated bare JSON objects, which is invalid.
- **`view_batch_permissive`** — sequential loop that records
  `(Vec<Bug>, Vec<BugViewFailure>)` in argument order. Calls
  `print_multi_bug_view(...)` once at the end; returns `Ok(())` even if
  every row failed. **Exception**: if a non-per-resource error
  (transport, auth, security — see [Error semantics](#error-semantics-and-exit-codes))
  occurs mid-loop, bail immediately with that error's code; do not
  continue.

## Output (table mode)

A new function in `src/output/bug.rs`:

```rust
pub fn print_multi_bug_view(rows: &[MultiBugRow], format: OutputFormat)
```

with an internal enum:

```rust
pub enum MultiBugRow {
    Ok(Bug),
    Failed { id: String, error: String },
}
```

For `Ok(bug)`, reuse the current `print_bug_detail` body (refactored to
take `&Bug` and write to `io::stdout()`; no behavior change for the
single-ID path).

For `Failed { id, error }`, emit a visually unambiguous block:

```text
Bug #999 — UNAVAILABLE
  Error: not found: 999
  Reason: bug is private or does not exist
─────────────────────
```

- `UNAVAILABLE` is rendered red+bold via the existing `colored` crate.
- The `Bug #N` prefix matches success blocks so a reader cannot
  mistake the row for a skipped argument.
- `Error:` carries the underlying error verbatim (NotFound, Api code
  102, etc.).
- `Reason:` is a short human gloss; for unmatched cases, omit the line.

Between every block — success or failure — emit `"─".repeat(60)`
(matches `print_history`). No trailing divider after the last block.

Single-ID `view` is **unchanged**: still calls `print_bug_detail`
directly with no divider and no `UNAVAILABLE` block.

## JSON shape

**Single-ID** (`bzr bug view 12345 --json`) — unchanged:

```json
{ "id": 12345, "summary": "...", "status": "...", ... }
```

**Multi-ID** (`bzr bug view 12345 my-alias 999 --permissive --json`):

```json
{
  "bugs":   [ { "id": 12345, ... }, { "id": 88, ... } ],
  "failed": [ { "id": "999", "error": "bug not found: 999" } ]
}
```

The wrapped shape is used for **every** multi-ID JSON invocation, not
only `--permissive` ones. `bzr bug view 1 2 3 --json` (no
`--permissive`, all succeed) emits `{"bugs": [...], "failed": []}` —
the `failed` array is always present for shape stability, and `jq`
consumers can rely on `.bugs[]` regardless of whether `--permissive`
was passed.

Two new types in `src/output/result_types.rs`, alongside `BatchResult`:

```rust
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct MultiBugViewResult {
    pub bugs: Vec<Bug>,
    pub failed: Vec<BugViewFailure>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BugViewFailure {
    pub id: String,    // String, not u64 — caller may have passed an alias
    pub error: String,
}
```

Why a new type instead of reusing `BatchResult`:

- `BatchResult` is mutation-flavored — `BatchResult::new` hardcodes
  `action: ActionKind::Updated`, and `ActionKind` has no `Viewed`
  variant. Adding one to satisfy a read is the wrong abstraction.
- `BatchFailure.id: u64` cannot carry an alias.
- `MultiBugViewResult` is `{bugs, failed}`, not
  `{resource, action, succeeded, failed}` — different domain.

## Error semantics and exit codes

Per Q4 (option B): `--permissive` truly suppresses per-bug failures.

| Invocation | Outcome | Exit code |
|---|---|---|
| 1 ID, succeeds | normal detail block | 0 |
| 1 ID, fails | propagate underlying error | NotFound→2, Auth→9, Api→4, Http→5, … |
| 1 ID + `--permissive` | rejected at handler entry | 7 (`InputValidation`) |
| N IDs, all succeed | N detail blocks + dividers | 0 |
| N IDs, any fails (no `--permissive`) | bail at first error | underlying error's code |
| N IDs + `--permissive`, partial fails | inline error blocks for failed rows | 0 |
| N IDs + `--permissive`, all fail | all rows are error blocks | 0 |

`--permissive` only suppresses **per-resource** errors. Session-level
failures still bail because they affect every subsequent call:
continuing produces only noise.

```rust
impl BzrError {
    /// Returns true for per-resource errors that may be suppressed
    /// under `--permissive` (e.g. one inaccessible bug among many).
    /// Session-level failures (transport, auth, security) always bail.
    pub fn is_per_resource(&self) -> bool {
        matches!(self, BzrError::NotFound { .. } | BzrError::Api { .. })
    }
}
```

| Variant | `--permissive` behavior |
|---|---|
| `NotFound` | suppress (per-resource) |
| `Api { code, .. }` | suppress (Bugzilla per-bug fault, e.g. 101 invalid, 102 access) |
| `Http`, `HttpStatus`, `XmlRpc` | bail (transport — every subsequent call would fail) |
| `Auth` | bail (session lost) |
| `PinMismatch`, `IssuerChanged` | bail (security — never silently swallow) |
| `DataIntegrity`, `Deserialize` | bail (server contract violated) |
| `Io`, `Keyring`, `Config`, `TomlParse`, `TomlSerialize` | bail |
| `InputValidation`, `BatchPartialFailure`, `Other` | not produced by `get_bug`; bail if observed |

## Tests

### Unit tests — `src/commands/bug/view_tests.rs` (sibling per CLAUDE.md)

- `view_single_unchanged` — one ID, success: detail block on stdout, exit 0. Single-ID byte-identical to today.
- `view_single_failure_propagates` — one ID, 404: `BzrError::NotFound`, exit 2.
- `view_multi_strict_all_succeed` — three IDs: three detail blocks separated by `─` divider, no trailing divider.
- `view_multi_strict_first_failure_bails` — three IDs, second 404s: first detail block printed, `Err(NotFound)`, no third call (assert wiremock).
- `view_multi_strict_json_all_succeed` — three IDs, JSON mode, all succeed: emits `{"bugs": [...], "failed": []}`, exit 0. Pins the wrapped-shape rule for non-permissive multi-ID JSON.
- `view_multi_strict_json_buffers` — three IDs, JSON mode, second fails: nothing on stdout, `Err` propagates. No partial JSON.
- `view_multi_permissive_partial` — three IDs, middle 404s: two detail blocks + one `UNAVAILABLE` block in argument order, exit 0.
- `view_multi_permissive_json` — same shape with `--json` returns `{"bugs": […], "failed": […]}`.
- `view_multi_permissive_all_fail` — every ID 404, all rows error blocks, exit 0, JSON `bugs: []`.
- `view_multi_permissive_with_alias` — alias + numeric + inaccessible: failure `id` preserves alias string verbatim.
- `view_permissive_single_id_rejected` — `--permissive` + 1 ID → `InputValidation`, exit 7, no HTTP call.
- `view_multi_permissive_transport_error_bails` — middle bug returns 500: bails despite `--permissive`, no inline placeholder.
- `view_multi_permissive_auth_error_bails` — middle bug returns Auth: bails despite `--permissive`.

Tests use the existing `wiremock` setup and `capture_stdout` helper. Each error scenario gets a mock returning the exact JSON Bugzilla emits for that case (404 with `{"error": true, "code": 101, …}`, etc.), exercising parsing→error mapping end-to-end.

### Output rendering — `src/output/bug_tests.rs`

- Divider count for N rows = N − 1; no trailing divider.
- Error block contains `UNAVAILABLE`, `Bug #<id>`, and `Error:` line.
- Single-row case (only one element passed to `print_multi_bug_view`) emits no divider.

### Functional test — `tests/functional/run-tests.sh` Phase 8 — Bugs

- Setup: three bugs against the running container; one group-restricted to a different account than the test runner.
- Run: `bzr bug view <ok1> <ok2> <restricted> --permissive`
- Assert:
  - exit 0
  - stdout contains `Bug #<ok1>` and `Bug #<ok2>` headers
  - stdout contains `Bug #<restricted> — UNAVAILABLE` and a non-empty `Error:` line
- Negative case: same command without `--permissive` exits non-zero (NotFound→2, or Api→4 depending on how the server reports the denial); the inaccessible bug must be the *first* in arg order so the strict bail is exercised.

## Documentation and changelog

- **`docs/bzr-cli.md`** — update the `bug view` section: variadic IDs, `--permissive`, multi-ID divider/error-block output, JSON shape switch.
- **`CHANGELOG.md`** — add under the next unreleased `## [X.Y.Z]` section in the same PR:

  ```
  - `bzr bug view` now accepts multiple IDs and a `--permissive` flag
    for partial results when some bugs are inaccessible (#156).
  ```

- **Man pages** are generated from clap doc-comments; the help-text
  rewrite covers them automatically. Confirm via the project's
  existing man-page generation step.
- **No on-disk state change**, no migration, no config knob.
- **Backward compatibility**: single-ID invocation (positional, table
  or JSON) is byte-identical to today, verified by
  `view_single_unchanged`.

## Open questions / risks

- **`Reason:` line content.** The design specifies a short human gloss
  for known cases (`bug is private or does not exist` for NotFound).
  For unmatched cases the line is omitted. The exact gloss strings can
  be finalized during implementation by inspecting actual server
  responses; they are not load-bearing for any test assertion.
- **XML-RPC mode.** The implementation reuses `client.get_bug()`, which
  already covers Hybrid and XML-RPC modes. No multi-ID-specific XML-RPC
  testing is in scope; the per-ID loop falls back automatically.
- **Performance.** N IDs = N round-trips. Acceptable for typical `view`
  use (a handful of bugs); if a future workflow needs hundreds, that
  motivates a separate spec for a server-side multi-ID path with its
  own error-fidelity tradeoffs.
