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
  (clap `conflicts_with`). A value of `-` reads from stdin, matching the
  `--description` convention. The comment is public; privacy is JSON-only (the
  flag form keeps a small surface).
- `--with-attachment <PATH>` — repeatable; `Vec<PathBuf>`. File is read from
  disk; content type is guessed from the extension by the existing
  `guess_content_type` helper unless overridden in JSON form.
- `--attachment-description <TEXT>` — repeatable; `Vec<String>`. The Nth value
  is the Nth attachment's summary. If fewer descriptions than attachments, the
  remaining attachments default their summary to the filename (the existing
  `attachment upload` default). **More descriptions than attachments is a
  validation error (exit 7)** — it signals a pairing mistake.
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

- `comment` — object: `{"body": <string, required>, "is_private": <bool,
  default false>}`. `deny_unknown_fields`.
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

### Flag form (single bug), full success

Unchanged from today: stdout prints `Created bug #N` (table) or the
`ActionResult` created object (JSON). The comment ID and attachment IDs are
*not* added to the success output — they are confirmable via `bug view`. (Keeps
the success shape stable; agents that need the IDs read them back.)

### Flag form (single bug), sub-step failure

- **stdout**: a `CompoundCreateResult` — the created bug ID plus a `failed`
  array of `{step, error}` sub-step failures (JSON), or a `Created bug #N` line
  (table). The result always carries the bug ID.
- **stderr**: one warning line per failed sub-step naming the bug ID, e.g.
  `warning: created bug #N but failed to add comment: <error>` and
  `warning: created bug #N but failed to upload attachment 'trace.log': <error>`.
- **exit**: `11`.

The acceptance criterion "the created bug ID is printed to stderr" is satisfied
by the warning line. The ID also appears on stdout in the result document.

### JSON array form

Reuses the existing `BatchCreateResult` shape, extended so a created-but-
partially-failed element is reported. A failed sub-step does **not** remove the
bug ID from `created`; instead the element is also recorded in `failed` with its
input index and the sub-step error. Exit 11 if any element has any failure
(create or sub-step).

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
file-read logic (extracted/shared). A file that cannot be read is an **input
validation** failure (exit 7), detected **before** the bug is created where
possible: in flag and JSON form, all attachment files are read and all params
built **before** the `create_bug` call. This keeps a missing-file typo from
filing a bug it then can't complete. (Comment bodies are likewise materialized
pre-create.) Only *server* sub-step failures (a POST that reaches the server and
fails) produce the compound partial-failure path.

### `bug clone` (TD-006)

`clone.rs` already prints `warning: created bug #N but failed to add the "Cloned
from bug #N" comment: <e>` on comment failure but returns `Ok`. Change: route
that failure through the shared report helper so it returns
`BatchPartialFailure { succeeded: 1, failed: 1 }` (exit 11). The success path
and the warning text are unchanged; only the return/exit changes.

## Shared infrastructure

A small `commands/bug/compound.rs` (or `runtime` helper) owns:

- `CompoundCreateResult` result type (bug ID + `Vec<SubStepFailure>`), in
  `output/result_types.rs` next to `BatchCreateResult`.
- A `run_sub_steps(client, bug_id, plan, w) -> Vec<SubStepFailure>` driver that
  posts the comment and attachments, emitting a stderr warning per failure and
  collecting the failures.
- The existing `ensure_batch_complete(succeeded, failed)` in `runtime::mutation`
  is reused to turn the failure count into the `BatchPartialFailure` error;
  `clone` and the compound create both call it.

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

## Test plan

- Unit (sibling `*_tests.rs`): flag→plan building, JSON schema parse
  (`deny_unknown_fields`, defaults), index-pairing of descriptions, the
  too-many-descriptions error, dry-run preview content.
- Wiremock (`#[tokio::test]`): the two issue-mandated scenarios plus attachment
  500, multi-attachment partial failure, and clone comment 500 → exit 11.
- Functional (`tests/functional/phases/`): compound create against a real
  container — success path (bug + comment + attachment, confirmed via
  `bug view` / `attachment list`) and `--dry-run`.
