# `bzr bug update`: post a comment atomically with field changes

**Date:** 2026-05-07
**Issue:** [#161](https://github.com/randomparity/bzr/issues/161)
**Parent spec:** `docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md` (Issue F)
**Status:** approved, pending implementation plan

## 1. Summary

Add `--comment <BODY>`, `--comment-file <PATH>`, and `--comment-private` to
`bzr bug update`. The comment is folded into the same `Bug.update` request
payload, so a status-change-with-explanation closes in one round-trip
instead of two (`bzr bug update` followed by `bzr comment add`). This is
the most common tester workflow regression flagged by the bzl-parity
review.

## 2. Background

Bugzilla's REST `PUT /rest/bug/{id}` accepts a nested `comment: {body,
is_private}` sub-object on the request body. `bzl-update` exploits this
(`reference/bzl/bzl-update:288-309`); `bzr bug update` does not. Today
testers must run two commands and there is no atomicity guarantee — the
field change can land while the comment fails (or vice versa).

`bzr` already has all the supporting machinery:

- `UpdateBugParams` (`src/types/bug.rs:501`) is the typed request body
  for `update_bug`.
- `BugzillaClient::update_bug` (`src/client/bug.rs:365`) does the PUT.
- `bzr bug create --description-file` (`src/cli/bug.rs:362-370`) is a
  precedent for file→string CLI input.
- `bzr comment add` (`src/commands/comment.rs:36-39`) is the precedent
  for empty-body rejection.

## 3. Scope

### In scope

- New CLI flags `--comment`, `--comment-file`, `--comment-private` on
  `bzr bug update`.
- New `CommentUpdate` type in `src/types/bug.rs`, plus a
  `comment: Option<CommentUpdate>` field on `UpdateBugParams`.
- Validation: file must exist and be UTF-8; empty / whitespace-only
  bodies rejected; `--comment-private` alone (no body source) rejected;
  `--comment` and `--comment-file` mutually exclusive.
- Batch behavior: the comment ships in every per-bug `PUT`, so each
  successful bug receives the same comment (matches bzl semantics).
- Output: table-mode success line gains a `(with comment)` suffix when
  a comment was posted; JSON output is unchanged.
- Tests: sibling unit tests in `update_tests.rs`, `bug_tests.rs`, and
  `bug_tests.rs` (client); functional tests in `tests/functional/` for
  Phase 9 (Comments).
- Docs: `docs/bzr-cli.md` updated with the three new flags;
  `CHANGELOG.md` entry under the next unreleased version.

### Out of scope

- `--private-comment <BODY>` (bzl shorthand) — superseded by
  `--comment X --comment-private`.
- `comment-is-private` — a separate bzl flag for *editing the privacy
  of existing comments*; different REST endpoint.
- Stdin / `$EDITOR` fallback for the comment body. `bzr bug update` is
  heavily scripted and a silent EDITOR launch would surprise pipelines;
  a body must be supplied explicitly via `--comment` or
  `--comment-file`.
- `--dupe-of` (issue G/#162), umbrella list-mutation flags (issue H),
  and other `bug update` field gaps — tracked separately.

## 4. CLI surface

Three new flags on `BugAction::Update` in `src/cli/bug.rs`:

```rust
/// Post a comment atomically with the field changes.
///
/// Mutually exclusive with `--comment-file`. Use `--comment-private`
/// to mark the comment private.
#[arg(long, value_name = "BODY", conflicts_with = "comment_file")]
comment: Option<String>,

/// Read the comment body from a UTF-8 file.
///
/// Mutually exclusive with `--comment`. The file must exist and be
/// readable; non-existent paths or non-UTF-8 contents fail with
/// exit code 7. Empty / whitespace-only contents are also rejected.
#[arg(long, value_name = "PATH", conflicts_with = "comment")]
comment_file: Option<std::path::PathBuf>,

/// Mark the comment private (visible only to users with elevated
/// permissions on the server).
///
/// Requires `--comment` or `--comment-file`; using `--comment-private`
/// alone is a usage error (exit 7).
#[arg(long)]
comment_private: bool,
```

The verbatim doc-comment on `Update` gains a paragraph between the
flag-syntax paragraph and the list-mutation paragraph:

> `--comment <BODY>` (or `--comment-file <PATH>`) posts a comment
> atomically with the field changes — a single `Bug.update` round-trip
> rather than a separate `bzr comment add` call. `--comment-private`
> marks it private. Empty / whitespace-only bodies are rejected
> (exit 7).

The "See also" footer drops the `bzr-comment-add(1) for adding a comment
as part of a status change` framing — the gap is closed; that page now
covers stand-alone comments only.

A new example is added to the verbatim doc-comment:

```text
bzr bug update 100 --status RESOLVED --resolution FIXED \
  --comment "Fixed by patch in #200"
```

## 5. Types

Add to `src/types/bug.rs`:

```rust
/// A comment to post atomically with a `Bug.update` call.
///
/// Serializes as `{"body": "...", "is_private": <bool>}`. Bugzilla's
/// REST `Bug.update` accepts this as a sub-object on the request,
/// which delivers the field changes and the comment in one round-trip.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct CommentUpdate {
    pub body: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_private: bool,
}
```

Extend `UpdateBugParams`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub comment: Option<CommentUpdate>,
```

Re-export in `src/types/mod.rs` next to `UpdateBugParams`:

```rust
StatusTransition, StringListUpdate, UpdateBugParams, CommentUpdate, FIELD_MAPPINGS,
```

Notes:

- `is_private: false` is skipped on the wire so request bodies stay
  minimal — matches how `StringListUpdate`'s empty fields are skipped
  today.
- `body` is owned `String` (not `&str`) so the lifetime survives batch
  iteration through `client.update_bug`.

## 6. Command wiring

Changes in `src/commands/bug/update.rs`:

1. **Destructure** `comment`, `comment_file`, `comment_private` in the
   pattern at the top of `build_update_params`.
2. **Resolve the body source** before constructing `UpdateBugParams`:
   - Both `comment` and `comment_file` are `None` and
     `comment_private` is `true` → `BzrError::InputValidation(
     "--comment-private requires --comment or --comment-file")`.
   - `comment` is `Some(s)` → use `s` directly.
   - `comment_file` is `Some(path)` → read via
     `std::fs::read_to_string(path)`. Map `io::Error` and non-UTF-8
     errors to `BzrError::InputValidation` with a path-bearing
     message (mirrors `--description-file` behavior in
     `bzr bug create`). The read helper is a 5-line local function
     in `update.rs` — not extracted to a shared module yet (we'll
     extract when a third consumer appears, e.g. `attachment upload
     --comment-file` from issue J).
   - Whitespace-only body → `BzrError::InputValidation(
     "empty comment, aborting")` (same wording as `bzr comment add`).
3. **Build `Option<CommentUpdate>`** and assign to `params.comment`.
   `is_private` comes from `comment_private`.

Output handling:

- `update_single`: when `params.comment.is_some()`, the table-mode
  message becomes `Updated bug #N (with comment)`. JSON output remains
  `ActionResult::updated(id, ResourceKind::Bug)` (no schema churn — the
  Bugzilla REST response does not return a comment ID for the inline
  comment, so a structured field would be a meaningless boolean).
- `update_batch`: in `print_batch_result`'s table branch, when a
  comment was posted, the existing `Updated bugs: #1, #2` line gains a
  ` (with comment)` suffix. Failed-bug lines are unchanged. Per-bug
  failures still surface with exit code 11.

## 7. Error handling and exit codes

No new `BzrError` variants. Existing variants cover all paths:

| Condition | Variant | Exit code |
| --- | --- | --- |
| `--comment` and `--comment-file` together | clap usage error | 2 |
| `--comment-private` without body source | `InputValidation` | 7 |
| `--comment-file` path missing | `InputValidation` | 7 |
| `--comment-file` not UTF-8 | `InputValidation` | 7 |
| Empty / whitespace body | `InputValidation` | 7 |
| Server rejects the comment (permissions, locked bug, etc.) | `Api` / `HttpStatus` | 4 / per-status |
| Batch: some bugs succeed, some fail | `BatchPartialFailure` | 11 |
| Batch: all bugs fail | first error's exit code | as today |

## 8. Tests

### Sibling unit tests — `src/commands/bug/update_tests.rs`

1. `build_update_params` carries the comment when `--comment "hi"` is
   passed (`params.comment == Some(CommentUpdate { body: "hi",
   is_private: false })`).
2. `--comment "hi" --comment-private` → `is_private: true`.
3. `--comment-file` with a UTF-8 tempfile → body equals contents.
4. `--comment-file` with a missing path → `BzrError::InputValidation`,
   path included in the message.
5. `--comment-file` with non-UTF-8 contents (`[0xff, 0xfe, 0xfd]`) →
   `InputValidation`.
6. Empty / whitespace-only body rejected (`--comment "   "` and
   `--comment-file` pointing at a whitespace file).
7. `--comment-private` alone → `InputValidation(
   "--comment-private requires --comment or --comment-file")`.
8. `--comment` and `--comment-file` together rejected at clap level
   (`Cli::try_parse_from` style if used elsewhere in the file).

### Sibling unit tests — `src/types/bug_tests.rs`

9. `CommentUpdate { body: "hi", is_private: false }` serializes to
   `{"body":"hi"}` (no `is_private` key).
10. `CommentUpdate { body: "hi", is_private: true }` serializes to
    `{"body":"hi","is_private":true}`.
11. `UpdateBugParams::default()` (with `comment: None`) emits no
    `comment` key.
12. `UpdateBugParams` with both `summary` and `comment` set emits both
    keys, no extras.

### Sibling unit test — `src/client/bug_tests.rs`

13. `update_bug` with a comment in params: wiremock matches `PUT
    /bug/{id}` with the nested `comment` object in the body; response
    success → `Ok(())`.

### Functional tests — `tests/functional/`

14. **Atomic field + comment** (Phase 9): `bzr bug update <id> --status
    RESOLVED --resolution FIXED --comment "see #other"`. Assert
    `bzr bug view <id>` shows `RESOLVED`/`FIXED` AND `bzr comment list
    <id>` shows the new comment, with a creation timestamp matching
    the update. This is the literal test plan from the issue.
15. **Private comment** (Phase 9): `bzr bug update <id> --comment "hi"
    --comment-private`. Assert `bzr comment list <id> --json |
    jq '.comments[-1].is_private'` is `true`.
16. **Batch atomicity** (Phase 9): `bzr bug update <id1> <id2>
    --comment "batch"`. Assert each bug's comment list grew by one.

## 9. Documentation

- `docs/bzr-cli.md` — `bzr bug update` section gains the three new
  flags and a short paragraph about atomicity.
- `CHANGELOG.md` — entry under the next unreleased `[X.Y.Z]`:
  `bug update: post a comment atomically with field changes via
  --comment/--comment-file/--comment-private (#161)`. Per the
  CLAUDE.md convention, the entry lands in the implementation PR
  alongside the code, not in a later release-prep commit.
- The verbatim doc-comment on `Update` (Section 4) is the
  authoritative source for `--help` and the generated man page.

## 10. Open questions

None. All design questions resolved during brainstorming:

- Q1 (stdin/`$EDITOR` fallback): no — strictly opt-in.
- Q2 (comment-only update with no field changes): allowed.
- Q3 (output): table-mode tag only; JSON unchanged.
- Q4 (file validation): mirrors `--description-file` behavior.
- Approach selection: nested `CommentUpdate` struct on
  `UpdateBugParams` (Approach 1).
- Helper extraction: keep the file-read helper local until a third
  consumer appears.
