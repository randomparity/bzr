# Resilient Response Envelopes — Design

**Issue:** [#135 — `attachment list` (and likely other commands) hard-fail with `missing field 'bugs'` on alternate response shapes](https://github.com/randomparity/bzr/issues/135)
**Date:** 2026-05-05
**Branch:** `fix/issue-135-resilient-envelopes`

## Problem

`bzr --json attachment list 218738` fails on `bugzilla.linux.ibm.com` (Bugzilla 5.0.6 with the `LTC` extension):

```text
{"error":{"exit_code":8,"message":"Failed to parse response: failed to deserialize response from
 https://bugzilla.linux.ibm.com/rest/bug/218738/attachment: missing field `bugs`","type":"deserialize"}}
```

Two underlying problems:

1. **No body context in the error.** The response body is only logged at `tracing::debug!` level (`src/client/mod.rs:321-329`). Diagnosing an envelope mismatch requires re-running with `-vv`.
2. **Brittle envelope shape.** `AttachmentBugResponse` (`src/client/attachment.rs:8-11`) requires a `bugs` field. Stock Bugzilla 5.0.6 returns both `bugs` and `attachments` keys; some deployment variants apparently return only `attachments`. The same brittleness exists in `CommentResponse` (`src/client/comment.rs:14-22`).

## Goals

- Surface a redacted body excerpt in `BzrError::Deserialize` so users can diagnose envelope mismatches in a single run.
- Accept alternate envelope shapes for `bug/<id>/attachment` and `bug/<id>/comment` without breaking existing handling.
- Establish a reusable pattern (`try_envelopes` helper) for future envelope-tolerance work.

## Non-goals

- No tolerant fallback for `BugListResponse` or `HistoryResponse` — `bugs` is the resource key, no plausible alt envelope exists.
- No XML-RPC fallback for attachments (that's [issue #133](https://github.com/randomparity/bzr/issues/133)).
- No new `BzrError` variant; `Deserialize` (exit code 8) is preserved.
- No CHANGELOG entry in this design doc — per project convention, the implementation PR adds the `## [Unreleased]` entry.

## Design

### Architecture

Two layered changes in `src/client/`:

**Change A — Body-preview diagnostics in `parse_json`.**
Both error paths in `BugzillaClient::parse_json` (`src/client/mod.rs:308-338`) append a redacted ~512-char body preview on a new line. Independent of Change B; ships value alone.

**Change B — Tolerant envelope dispatch.**
A new helper `try_envelopes` accepts a parsed `serde_json::Value` and a list of `(envelope_key, extractor_fn)` candidates, returning the first that deserializes. Two endpoints adopt it:

| Endpoint | Primary envelope | Alt envelope |
|---|---|---|
| `GET /rest/bug/<id>/attachment` | `bugs.<id>: [Attachment]` | `attachments: [Attachment]` |
| `GET /rest/bug/<id>/comment` | `bugs.<id>.comments: [Comment]` | `comments: [Comment]` |

Bug search and history get only Change A.

### Component changes

#### `src/client/mod.rs`

- **New private helper** `format_body_preview(body: &str) -> String`:
  - Truncates to 512 chars on a UTF-8 char boundary.
  - Appends `…` if truncated.
  - Runs through `redact_api_key` (lifted from `error.rs` to `crate::http`).
  - Collapses internal newlines and tabs to single spaces so the preview stays compact.
- **Modified** `parse_json`:
  - On `serde_json::from_str` failure (line 321-329), append `\nbody preview (N chars): <preview>`.
  - On `serde_json::from_value` failure (line 333-337), same.
- **New crate-private primitive** `get_json_value(path) -> Result<serde_json::Value>`:
  - Sends GET, runs `parse_json`'s read+parse+`check_bugzilla_200_error` flow, returns the `Value`.
  - `get_json::<T>` becomes a thin wrapper: `get_json_value().and_then(serde_json::from_value)` (with the same body-preview error mapping).
- **New crate-private helper** `try_envelopes`:
  ```rust
  fn try_envelopes<T>(
      value: serde_json::Value,
      candidates: &[(&'static str, fn(serde_json::Value) -> Result<T>)],
  ) -> Result<T>
  ```
  - Inspects top-level keys; tries candidates whose key is present, then any remaining.
  - On total failure, returns the *first* candidate's error (stable message).
  - The `envelope_key` strings are included in the failure message: `"tried envelopes: bugs, attachments"`.
  - Re-serializes the parsed `Value` to produce a body preview (so the no-matching-envelope error contains the same diagnostic info as a `parse_json` typed-deserialize failure).

#### `src/client/attachment.rs`

`get_attachments` switches from `get_json::<AttachmentBugResponse>` to `get_json_value` + `try_envelopes` with two extractors:

- `("bugs", extract_bugs_attachments)` — deserializes as `AttachmentBugResponse`, returns first `HashMap` entry's `Vec`.
- `("attachments", extract_flat_attachments)` — deserializes as `{ attachments: Vec<Attachment> }`, returns the vec.

#### `src/client/comment.rs`

`get_comments_since_rest` does the analogous swap with `("bugs", …)` (current `CommentResponse`) and `("comments", …)` (`{ comments: Vec<Comment> }`) extractors.

#### `src/error.rs` / `src/http/mod.rs`

`redact_api_key` moves from `error.rs` (private) to `crate::http` (`pub(crate)`), so both `format_http_error` and the new `format_body_preview` can call it. `error.rs`'s existing tests stay where they are and call the new path.

### Behavior matrix

| Server response | Old behavior | New behavior |
|---|---|---|
| Stock 5.0.6 (`bugs` populated, empty `attachments`) | Works | Works (matches `bugs` first) |
| IBM-style (`attachments` populated, no `bugs`) | `Deserialize: missing field 'bugs'` | Works (falls back to `attachments`) |
| Both keys populated | Works | Works (matches `bugs` first; ignores `attachments`) |
| Truly malformed (HTML error page, garbage) | `Deserialize: ...` (no body context) | `Deserialize: ... \nbody preview (512 chars): <html>...` |
| Bugzilla 200-with-error envelope | Handled by `check_bugzilla_200_error` → `BzrError::Api` | Unchanged — `check_bugzilla_200_error` runs before envelope dispatch |
| Body contains `Bugzilla_api_key=secret` | N/A (body not in error) | API key redacted in preview |

### Error handling

- No new `BzrError` variant; `BzrError::Deserialize` and exit code 8 unchanged.
- `try_envelopes` returns the first candidate's error on total failure, so the error message is stable across repeated runs.
- `check_bugzilla_200_error` continues to run in `get_json_value` before envelope dispatch — a 200-with-error response surfaces as `BzrError::Api`, not `BzrError::Deserialize`.

### Testing

Seven new wiremock-based unit tests, colocated in their respective `#[cfg(test)] mod tests`:

**`src/client/mod.rs`:**
1. `parse_json_includes_body_preview_on_typed_failure` — valid JSON, wrong shape; assert error contains `body preview` and JSON keys.
2. `parse_json_includes_body_preview_on_invalid_json` — malformed body; assert preview present.
3. `parse_json_redacts_api_key_in_body_preview` — body contains `Bugzilla_api_key=secret`; assert `secret` absent, `[REDACTED]` present.
4. `parse_json_truncates_long_body_preview` — 2KB JSON body; assert preview ≤ ~520 chars and ends with `…`.

**`src/client/attachment.rs`:**
5. `get_attachments_accepts_bugs_envelope` — covers existing path explicitly.
6. `get_attachments_accepts_attachments_envelope` — `{"attachments":[{...}]}` with no `bugs` key; assert `Vec<Attachment>` returned.

**`src/client/comment.rs`:**
7. `get_comments_since_rest_accepts_comments_envelope` — `{"comments":[{...}]}` with no `bugs` key; assert `Vec<Comment>` returned.

No functional/integration test changes. We don't have an IBM-LTC fixture in `tests/functional/versions/`, and both code paths are covered at the unit level.

### Files touched

- `src/client/mod.rs` — `parse_json` body preview, `get_json_value`, `try_envelopes`, `format_body_preview`, four new tests.
- `src/client/attachment.rs` — `get_attachments` rewrite, two new tests.
- `src/client/comment.rs` — `get_comments_since_rest` rewrite, one new test.
- `src/error.rs` — `redact_api_key` moves out; existing tests follow the new path.
- `src/http.rs` — receives `redact_api_key` (it already holds the `Bugzilla_api_key` constant).

No public API change.

## Rollout

Single PR. The diagnostics change is independently valuable and committed first; the tolerant-envelope changes follow as a second commit on the same branch. CHANGELOG entry under `## [Unreleased]` (Fixed): "attachment list and comment list now accept alternate response envelopes returned by some Bugzilla 5.0.x deployments (e.g. IBM LTC). Deserialization failures now include a redacted body preview to aid diagnosis."
