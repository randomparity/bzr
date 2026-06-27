# Issue #458 — Compound bug create (`--with-comment` / `--with-attachment`)

Status: Accepted
Issue: https://github.com/randomparity/bzr/issues/458
Related ADR: [0012](../../adr/0012-compound-create-report-not-rollback.md)

## Problem

Filing a bug with its first comment and an attachment today takes three separate
`bzr` process invocations: `bug create`, then `comment add <id>`, then
`attachment upload <id>`. The `bzr-file-bug` and `bzr-triage-bug` skills both
document this multi-step sequence. The failure mode is the agent footgun the
issue targets: if `bug create` succeeds but a follow-up call fails, the agent
that drove the three calls separately may not have captured the new bug ID, has
no way to recover it, and re-files — creating a duplicate. The same class of bug
is already tracked as TD-006 for `bug clone`, whose "Cloned from bug #N"
back-reference comment is non-atomic with the create.

## Goal

Let one `bug create` invocation file the bug **and** its first comment **and**
one or more attachments. When every sub-step succeeds, behavior and output are
the existing single-create contract. When a *sub-step after the bug create*
fails, the new bug ID is never lost: it is reported on stdout (the result
document) and named in a stderr warning, and the process exits
`11` (`BatchPartialFailure`). The created bug is **not** deleted.

Two entry points:

1. **Flag form** — `bug create` gains:
   - `--with-comment <text>` / `--with-comment-file <path>` (mutually exclusive):
     post one comment on the new bug after create.
   - `--with-attachment <path>` (repeatable): upload each file after create.
   - `--attachment-description <text>` (repeatable): the Nth value is the summary
     of the Nth `--with-attachment`; index-paired.

2. **JSON form** — `--from-json` gains two optional keys per bug object:
   - `comment`: `{"body": "..."}` (or `{"body": "...", "is_private": true}`).
   - `attachments`: array of `{"file": "...", "description": "...",
     "content_type": "...", "is_patch": bool, "is_private": bool}`.
   The array (batch) form applies these per element; per-element sub-step
   failures roll into the existing partial-failure result and exit 11.

TD-006 is resolved by routing `bug clone`'s back-reference comment through the
same report path, so a failed clone comment now also exits 11 with the ID
surfaced.

## Non-goals

- **No rollback / bug deletion.** Bugzilla REST has no multi-resource
  transaction and no safe bug-delete; deleting a just-created bug to "undo" a
  failed comment is more destructive than leaving a filed bug the agent can
  finish manually. See ADR-0012. "Transactional" in the issue title is satisfied
  by *never losing the ID*, not by atomic rollback.
- **No reordering of sub-steps.** Create → comment → attachments, in that order,
  sequentially. The bug must exist before a comment or attachment can target it.
- **No new client API methods.** The compound verb composes the existing
  `create_bug`, `add_comment`, and `upload_attachment` client calls.
- **No partial-success exit 0.** Any sub-step failure is a partial failure
  (exit 11), even though the bug itself was created. The bug ID is the recovery
  handle, not a success signal.

## Surface

### Flag form

```
bzr bug create --product P --component C --summary "..." \
  --with-comment "Reproduced on F42; root cause is X." \
  --with-attachment trace.log --attachment-description "boot trace" \
  --with-attachment dmesg.txt  --attachment-description "dmesg tail"
```

- `--with-comment <text>` / `--with-comment-file <path>` — mutually exclusive
  (clap `conflicts_with`). `--with-comment` is literal text; `--with-comment-file`
  reads a UTF-8 file path. **Neither accepts `-` / stdin**: the create flow
  already consumes stdin for the description (`create.rs` piped-stdin path) and
  stdin is single-consumer, so a stdin-reading comment would collide with a
  piped description. Comment-from-stdin is available via the JSON form instead.
  An empty / whitespace-only comment body is rejected pre-create (exit 7), like
  the empty-description guard. The comment is public; privacy is JSON-only (the
  flag form keeps a small surface). Supplying `--with-comment` does **not** alter
  description resolution — the `$EDITOR` flow still triggers on a TTY with no
  description.
- `--with-attachment <PATH>` — repeatable; `Vec<PathBuf>`. File is read from
  disk; content type is guessed from the extension by the existing
  `guess_content_type` helper unless overridden in JSON form.
- `--attachment-description <TEXT>` — repeatable; `Vec<String>`. The Nth value
  is the Nth attachment's summary. If fewer descriptions than attachments, the
  remaining attachments default their summary to the filename (the existing
  `attachment upload` default). An explicit empty / whitespace-only description
  is treated as absent (falls back to the filename), not an error. **More
  descriptions than attachments is a validation error (exit 7)** — it signals a
  pairing mistake.
- `--attachment-description` without any `--with-attachment` is a validation
  error (exit 7).
- These flags are rejected with `--from-json` (`conflicts_with = "from_json"`):
  the JSON object carries its own `comment`/`attachments`, so mixing the two
  comment/attachment sources is ambiguous. (The existing scalar field flags
  still overlay onto JSON; only the compound flags conflict.)

### JSON form

```
echo '{
  "product": "P", "component": "C", "summary": "...",
  "description": "...",
  "comment": {"body": "Follow-up note", "is_private": false},
  "attachments": [
    {"file": "trace.log", "description": "boot trace", "content_type": "text/plain"},
    {"file": "patch.diff", "description": "fix", "is_patch": true}
  ]
}' | bzr bug create --from-json -
```

- `comment` — object: `{"body": <string, required, non-empty>, "is_private":
  <bool, default false>}`. `deny_unknown_fields`. An empty / whitespace-only
  `body` is rejected pre-create (exit 7).
- `attachments` — array of objects:
  - `file` — required; path read from disk.
  - `description` — optional; attachment summary. Defaults to the filename.
  - `content_type` — optional; defaults to the extension guess.
  - `is_patch` — optional bool, default false.
  - `is_private` — optional bool, default false.
  - `deny_unknown_fields`.
- Both keys default to absent (`#[serde(default)]`), so existing single-object
  and array `--from-json` payloads are byte-for-byte unaffected.
- In the **array** form, each element may carry its own `comment`/`attachments`;
  a sub-step failure on element *i* marks that element failed (the bug for *i*
  was still created — its ID is in the result), and the batch exits 11.

## Output contract

The compound path has **two scopes** — *single* (flag form OR single-object
`--from-json`) and *array* (`--from-json` array). Both share one sub-step
failure record so an agent parses the same `{step, error}` shape in either:

```rust
// step ∈ {"comment", "attachment"}; `file` present only for attachments.
struct SubStepFailure { step: String, file: Option<String>, error: String }
```

### Single scope (flag form and single-object `--from-json`)

Both inputs run the **same compound driver** and emit the same shapes — there is
no flag-vs-JSON divergence (resolves the under-specified single-object path:
`create_json::handle`'s `One` arm calls the shared driver, not the plain
`create_and_report`).

- **Full success** — unchanged from today: stdout prints `Created bug #N`
  (table) or the plain `ActionResult` created object (JSON). The new compound
  result type is **not** emitted on success, so the success shape is byte-for-
  byte stable. Comment/attachment IDs are confirmable via `bug view`.
- **Sub-step failure** — stdout prints a `CompoundCreateResult`:
  ```json
  {"resource":"bug","action":"created","id":N,
   "failed":[{"step":"comment","error":"..."},
             {"step":"attachment","file":"trace.log","error":"..."}]}
  ```
  (table form: `Created bug #N`). stderr prints one warning per failed sub-step
  naming the bug ID (`warning: created bug #N but failed to add comment: <e>`,
  `warning: created bug #N but failed to upload attachment 'trace.log': <e>`).
  Exit `11`, via `ensure_batch_complete(succeeded = 1, failed = failed.len())`.

The acceptance criterion "the created bug ID is printed to stderr" is satisfied
by the warning line; the ID is also on stdout in the result.

### Array scope (`--from-json` array)

Keeps the existing `BatchCreateResult { created: [u64], failed: [CreateFailure]
}`. To carry sub-step failures, `CreateFailure` gains three optional fields,
each `#[serde(skip_serializing_if = "Option::is_none")]` so **existing create-
failure JSON is byte-for-byte unchanged**:

```rust
struct CreateFailure {
    index: usize,
    bug_id: Option<u64>,   // present iff the bug was created but a sub-step failed
    step: Option<String>,  // "comment" | "attachment"; absent for a create failure
    file: Option<String>,  // attachment filename, when step == "attachment"
    error: String,
}
```

- A **create** failure (bug never filed): `{index, error}` — exactly as today.
- A **sub-step** failure (bug filed, comment/attachment failed): the new bug ID
  is added to `created` **and** a `{index, bug_id, step, [file], error}` entry is
  added to `failed`. One element may produce several `failed` entries (e.g. a
  failed comment and a failed attachment).
- **Invariant (documented for consumers): `created` and `failed` are NOT
  disjoint.** `created` lists every bug the server actually filed; `failed`
  lists every failure. A created-but-partially-failed element appears in both.
  Counting filed bugs uses `created.len()`; detecting any problem uses
  `failed`. This is a compatibility change from today's disjoint behavior and is
  called out in the CHANGELOG.
- Exit `11` via `ensure_batch_complete(succeeded = created.len(), failed =
  failed.len())` if any element had any failure (create or sub-step).

### `--dry-run`

Prints the full planned payload without writing: bug fields **plus** the
resolved comment body and the list of attachments (filename, summary,
content_type, sizes), marked `"action":"dry-run"`. No network calls; attachment
files are read (to report size and validate readability) but nothing is posted.
The array form prints one coherent dry-run object for the batch as today.

## Behavior details

### Sub-step ordering and failure handling

For one bug:

1. `create_bug(params)` → `id`. A failure here is a normal create error (exit 4
   / etc.), no compound semantics — nothing was created.
2. If a comment is configured: `add_comment(id, ...)`. On error, record a
   `comment` sub-step failure and **continue** to attachments (do not abort —
   the agent may still want the attachments uploaded; all failures are reported
   together).
3. For each attachment, in order: `upload_attachment(...)`. On error, record an
   `attachment` sub-step failure (naming the file) and continue to the next
   attachment.
4. After all sub-steps: if any failed, write the result, emit the warnings, and
   return `BatchPartialFailure { succeeded, failed }` (exit 11). Otherwise write
   the normal success result.

Continuing past the first sub-step failure (rather than short-circuiting) means
the agent gets a complete failure report in one run instead of discovering
failures one at a time across retries.

### Attachment reading

Attachment files are read with the existing `prepare_upload` content-type and
file-read logic (extracted/shared). A file that cannot be read surfaces as an
**I/O error (exit 6)**, consistent with `attachment upload`'s missing-file
behavior (the shared helper maps the read error the same way). It is detected
**before** the bug is created: in flag and JSON form, all attachment files are
read and all params built **before** the `create_bug` call. This keeps a
missing-file typo from filing a bug it then can't complete. (Comment bodies are
likewise materialized pre-create; an empty comment body is exit 7.) Only
*server* sub-step failures (a POST that reaches the server and fails) produce the
compound partial-failure path.

### `bug clone` (TD-006)

`clone.rs` already prints `warning: created bug #N but failed to add the "Cloned
from bug #N" comment: <e>` on comment failure but returns `Ok`. Change: route
that failure through the shared report helper so it returns
`BatchPartialFailure { succeeded: 1, failed: 1 }` (exit 11). The success path
and the warning text are unchanged; only the return/exit changes.

## Shared infrastructure

A small `commands/bug/compound.rs` owns:

- `SubStepFailure { step, file, error }` and `CompoundCreateResult { id,
  failed: Vec<SubStepFailure>, .. }`, in `output/result_types.rs` next to
  `BatchCreateResult`. `CreateFailure` is extended with the optional
  `bug_id`/`step`/`file` fields there too.
- A `CompoundPlan { comment: Option<AddCommentParams>, attachments:
  Vec<UploadAttachmentParams> }`, built **before** any network call from
  validated flag/JSON input (files read, bodies materialized, emptiness checked).
- A `run_sub_steps(client, bug_id, &plan, w) -> Vec<SubStepFailure>` driver that
  posts the comment then each attachment, emitting a stderr warning per failure
  (naming the bug ID) and collecting the failures. Shared by the single-scope
  flag/JSON handlers and reused conceptually by the array loop.
- The existing `ensure_batch_complete(succeeded, failed)` in `runtime::mutation`
  turns the failure count into `BatchPartialFailure`; `clone`, the single-scope
  compound create, and the array loop all funnel through it.

## Acceptance criteria (from the issue) → coverage

- `--with-comment` posts the comment; comment failure keeps the ID → flag-form
  handler + wiremock test (create 201, comment 500 → exit 11, ID on stderr).
- `--with-attachment --attachment-description` uploads after create; failure
  prints ID, exit 11 → flag-form handler + wiremock test.
- `--from-json` accepts `comment`/`attachments`; array per-element failures exit
  11 and report each → JSON handler + wiremock tests (object form; array form
  one good + one failing comment).
- `--dry-run` prints full planned payload → dry-run path + test asserting comment
  + attachments in the preview.
- Exit 11 on any sub-step failure, ID on stderr → shared helper + tests.
- Wiremock: create ok + comment 500 → exit 11, ID in stderr → test.
- Wiremock: array one good + one failing comment → exit 11, good ID reported →
  test.
- `docs/bzr-cli.md` + `bzr-file-bug` skill updated → docs tasks.
- TD-006 resolved via shared helper; clone surfaces ID on partial failure → clone
  change + test asserting exit 11.

## Resolved ambiguities

- **Rollback vs report** — report-and-continue, no deletion (ADR-0012);
  acceptance criteria define it, repo precedent (clone, attachment upload)
  confirms it.
- **Multiple attachments in flag form** — `--with-attachment` repeatable,
  `--attachment-description` index-paired; extra descriptions error (exit 7).
- **`bug clone` exit code on comment failure** — now exit 11 (shared helper),
  a deliberate behavior change for partial-failure consistency.
- **`description` → attachment field** — maps to the attachment *summary* (the
  Bugzilla field shown in the UI), matching `attachment upload --summary`.
- **Comment stdin** — `--with-comment`/`--with-comment-file` do **not** accept
  `-`/stdin (single-consumer collision with the piped description); use the JSON
  form for comment-from-stdin.
- **Single vs array result shapes** — single scope emits `ActionResult` on
  success / `CompoundCreateResult` on failure; array scope keeps
  `BatchCreateResult` with an extended `CreateFailure`. Both share the
  `{step, file, error}` sub-step record. `created`/`failed` are non-disjoint in
  the array scope (documented compat change).
- **Empty inputs** — empty comment body → exit 7; empty attachment description →
  falls back to filename.

## Test plan

- Unit (sibling `*_tests.rs`): flag→plan building, JSON schema parse
  (`deny_unknown_fields`, defaults), index-pairing of descriptions, the
  too-many-descriptions error, empty-comment-body → exit 7, empty-description
  fallback to filename, `--attachment-description` with no `--with-attachment`
  → exit 7, dry-run preview content (comment + attachments present).
- Wiremock (`#[tokio::test]`): the two issue-mandated scenarios plus attachment
  500, multi-attachment partial failure, and clone comment 500 → exit 11.
- Functional (`tests/functional/phases/`): compound create against a real
  container — success path (bug + comment + attachment, confirmed via
  `bug view` / `attachment list`) and `--dry-run`.
