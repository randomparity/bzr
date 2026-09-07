# 0063 — Multi-bug attachment upload publishes a batch result

- Status: Accepted
- Date: 2026-09-07
- Issue: #674
- Related: [0007](0007-json-output-schema-version-envelope.md), [0015](0015-server-errors-are-never-masked.md)

## Context

python-bugzilla's `bugzilla attach ID1 ID2 -f FILE` uploads one file to several
bugs in one invocation. `bzr attachment upload` takes exactly one bug:
`UploadArgs.bug_id: u64` (`src/cli/attachment.rs`). The comparison suite records
the difference as an expected gap
(`tests/functional/compare/03-attachments.sh`, `compare/03-attachments/multi-bug-upload`).

Widening the positional to a list forces a result-shape decision. The constraint
is not arity: ADR 0007's envelope wraps an array payload perfectly well —
`bzr schema` returns a bare `Vec<&str>` through the same writer
(`src/commands/schema.rs`), and `write_ndjson` (`src/output/formatting.rs`)
streams one line per array element under `--output ndjson`. The constraint is
that the result has to carry per-bug **failures**, and a sub-step marker for the
`--comment-private` follow-up, alongside the successes. The existing
`UploadResult` (`schemas/upload-result.json`) carries a scalar `bug_id` beside a
scalar attachment `id`; an array of them has nowhere to put either.

The command is also not idempotent. Bugzilla has no attachment-delete call, so
an unwanted attachment can only be marked obsolete. Anything that causes a
second upload to the same bug leaves a residue the caller cannot remove.

## Decision

**`bzr attachment upload <BUG_ID>... <FILE>` accepts one or more bug IDs. One
bug keeps today's `UploadResult`; two or more emit a new published
`attachment-upload-batch-result`.**

Three properties are deliberate:

- **The single-bug result shape does not move.** One bug ID produces the same
  `UploadResult` object and the same
  `Uploaded attachment #N to bug #M (X bytes)` line it produces today. Every
  invocation in `docs/bzr-cli.md` keeps working, because each of them writes the
  file immediately after the bug. **The argument grammar does move**, and the two
  consequences below name exactly how.

- **The batch shape pairs each bug with its attachment.** `uploaded` is an array
  of `{bug_id, attachment_id}` objects rather than a bare ID list, and `size`
  is stated once because one file is uploaded to every target. `failed` entries
  are `{bug_id, error}` with an optional `step: "comment_private"` marking the
  case where the attachment was created (so the bug also appears in `uploaded`)
  and only the follow-up privacy flip failed. That optional `step` field is the
  same device `schemas/batch-result.json` already uses for `comment_tags`.
  Any per-bug failure returns `BatchPartialFailure` (exit 11) through the
  shared `ensure_batch_complete`, matching `bug update`'s batch path and
  `attachment download`'s bulk path.

- **Duplicate bug IDs are rejected before any request.** `validate_action`
  returns `BzrError::input` (exit 7) naming the repeated ID. The upload loop
  never has to decide what a repeat means, and a typo cannot leave an
  undeletable second attachment on a bug.

## Consequences

- **An option may no longer sit between the bug IDs and the file.**
  `bzr attachment upload 12345 --summary s patch.diff` parses today and becomes
  a clap `UnknownArgument` (exit 2): the option terminates the variadic, and the
  token after it has no positional slot left. `upload 12345 patch.diff --summary s`
  and `upload --summary s 12345 patch.diff` both still parse. Every invocation in
  `docs/bzr-cli.md` is of the surviving form, so no documented example breaks, but
  a script using the interleaved spelling does. This is the price of taking the bug
  IDs positionally at all; the alternatives below show it is not avoidable by
  rearranging the positionals. Measured with a probe binary built against clap
  `=4.6.6` on macOS arm64:

  | invocation | before | after |
  |---|---|---|
  | `upload 1 f.txt --summary s` | ok | ok |
  | `upload --summary s 1 f.txt` | ok | ok |
  | `upload 1 --summary s f.txt` | ok | `UnknownArgument` |
  | `upload 1 --flag 'review?' f.txt` | ok | `UnknownArgument` |

- **With an all-numeric argument list, the last token is always the file.**
  `bzr attachment upload 12345 67890` parses as bug `12345` and a file named
  `67890` — the same probe returns `bug_ids=[1] file="2"` for `upload 1 2`.
  A forgotten file argument, or a script whose `"$FILE"` expanded empty and was
  dropped, therefore silently removes the last bug from the target set rather
  than failing. Usually the result is exit 7 because no such file exists; where a
  file named for a bug number does exist in the working directory, it uploads the
  wrong bytes to the remaining bug, and this ADR's own context establishes there
  is no way to remove it afterwards. Only the one-token form `upload 12345` is a
  clean `MissingRequiredArgument`. The behaviour is pinned by a test rather than
  left to be discovered.

- **`SCHEMA_VERSION` moves `3.0.3` → `3.0.4`.** A new published schema is an
  additive change under ADR 0007's semver policy. The constant lives in
  `src/output/mod.rs`, and the value is pinned by hand in `README.md`,
  `docs/bzr-cli.md`, `content/skills/bzr-reference/reference/commands.md`,
  `content/skills/bzr-reference/reference/json-recipes.md`,
  `content/skills/bzr-dependency-analysis/scripts/collect.py` and its two test
  fixtures, and four functional phase scripts. Every one moves in this change or
  the envelope phases fail.

- **A partial fan-out is not rolled back.** If bug 1 accepts the attachment and
  bug 2 refuses it, bug 1 keeps its attachment and the command exits 11 with
  both outcomes in the result. Rollback would mean deleting an attachment, which
  Bugzilla does not offer, so the alternative to reporting is hiding. The same
  reasoning already governs `attachment upload --comment-private`, which leaves
  the attachment in place when the privacy flip fails.

- **The single-bug and multi-bug failure exit codes differ.** One bug that fails
  exits with the underlying error's own code (4 for an API refusal, and so on);
  two bugs where one fails exit 11. That is the split `attachment download`
  already has between its single and bulk shapes, and it follows from the batch
  result being able to carry more than one outcome.

- **A `comment_private` failure is narrated differently in table mode.** Such an
  entry means the attachment exists, so rendering it as `Failed to upload to
  bug #M` would point the operator at a retry — the one action that leaves an
  undeletable second attachment. The table renderer branches on `step` and says
  the attachment landed but the comment could not be made private, and the batch
  path suppresses `flip_new_comment_private`'s own `warn_partial` pair so one
  failure produces one message rather than three.

- **Such a bug is listed in `uploaded` and still counts as a failed target.**
  The two are not in tension, and the pairing is the reason `step` exists: the
  bug appears in `uploaded` because its `attachment_id` is the caller's handle
  for setting the privacy by hand, and it counts on the failed side of
  `BatchPartialFailure` because it did not fully succeed. Each bug contributes at
  most one `failed` entry and duplicates are rejected before the loop, so the two
  counts always sum to the number of bugs. Two bugs where one flip fails report
  `succeeded: 1, failed: 1`, not `2` and `0` — a count that excluded sub-step
  failures could never reach exit 11 at all, which would contradict the exit rule
  above and leave the batch path exiting 0 on a failure the single-bug path
  already reports.

- **`BzrError::BatchPartialFailure`'s message reads `batch update: N succeeded,
  M failed`** (`src/error.rs`), which is inaccurate for an upload. The variant is
  shared with `bug update` and its text is not changed here; the accurate
  per-bug detail is on stdout in the result body, which is where a caller reads
  it. Renaming the message is a cross-command change this decision does not own.

- **`attachment download`'s `AttachmentBatchResult` stays unpublished.**
  Publishing this result does not publish that one, and the inconsistency is
  recorded rather than fixed here — closing it means writing a schema for a
  shape this change does not touch.

- **CHANGELOG entry required.** The positional arity of a documented command
  changes, so the change needs a `feat` commit with a non-infra scope; release
  notes are generated from subject lines only.

## Alternatives considered

- **Emit the batch shape for every invocation, one bug included.** judgment: it
  breaks the `--json` contract every existing caller of the common case reads,
  in exchange for a uniformity nobody asked for.
- **Emit an array of per-bug records under the existing envelope, unpublished.**
  verified: the envelope carries array payloads already — `bzr schema` writes a
  bare `Vec<&str>` through `write_result` (`src/commands/schema.rs`), and
  `write_ndjson` (`src/output/formatting.rs`) streams one compact line per array
  element for `--output ndjson`. So arity was never the obstacle. Rejected because
  an array of `UploadResult` has nowhere to carry a `failed[]` entry or the
  `comment_private` sub-step marker, and a per-target record that can carry them
  is a new shape either way — at which point publishing it costs one file.
- **Reuse `batch-result`.** verified: `schemas/batch-result.json` declares
  `succeeded` as `{"type": "array", "items": {"type": "integer"}}`, so it can
  carry bug IDs or attachment IDs but not the pairing — a caller could not learn
  which attachment landed on which bug, which is the one fact a fan-out adds.
- **Reuse `batch-create-result`.** verified: `schemas/batch-create-result.json`
  declares `created` as a bare integer array and fixes `failed[].step` to
  `["comment", "attachment"]`. Same pairing loss, plus an enum that describes
  `bug create --from-json`'s sub-steps rather than this command's.
- **Leave the batch result unpublished, following `AttachmentBatchResult`.**
  verified: `AttachmentBatchResult` has no entry in the `SCHEMAS` registry
  (`src/commands/schema.rs`), so the precedent is real and this option costs no
  version bump. Rejected because the same file's module documentation states the
  registry exists so agents can "validate against a contract instead of
  branching per command"; the download gap is debt to close, not a policy to
  extend to a shape being introduced now.
- **Accept duplicate bug IDs and upload once per occurrence.** verified: the
  Bugzilla API this command uses (`Bug.add_attachment`) has no delete
  counterpart, and `bzr attachment update --obsolete`
  (`src/cli/attachment.rs`) is the only remedy `bzr` can offer for an
  unwanted attachment. judgment: a repeated ID is a typo far more often than an
  instruction to attach the file twice.
- **Take the bugs through a repeatable `--bug` flag instead of positionals.**
  judgment: it would make `attachment upload` disagree with itself — the bug is
  positional today — and with every documented example, to avoid a parsing
  problem that does not exist.
- **Collect every positional into one `Vec<String>` and split the last off as the
  file.** verified: a probe binary built against clap `=4.6.6` on macOS arm64 with
  a single `num_args = 2..` positional rejects `upload 1 --summary s f.txt`,
  `upload 1 --flag 'review?' f.txt`, **and** `upload 1 2 --summary s f.txt`, all
  as `TooFewValues`. Rejected because it is strictly worse than two positionals:
  it breaks the interleaved spelling just the same, breaks the multi-bug
  interleaved spelling too, and gives up clap's `u64` parsing and usage rendering
  for the bug IDs.
- **Move the file to a `-f/--file` flag, as python-bugzilla spells it.**
  verified: clap `=4.6.6` parses a variadic positional followed by a required
  positional and renders `Usage: ... [OPTIONS] <BUG_ID>... <FILE>`, confirmed by
  the same probe, so the existing positional order survives the widening.
  Rejected to keep every documented invocation and every script using the
  file-after-bug spelling working; epic #665 already treats option syntax as
  matched by capability rather than spelling. The tradeoff is real rather than
  free: a flag-carried file has no boundary to guess, so it would turn both
  consequences above — the interleaved-option break and the omitted-file
  ambiguity — into clean usage errors. Compatibility with the documented
  spelling is what is being bought, and those two behaviours are the price.

## Amendment (2026-09-07): a flip failure stops the fan-out, and the batch gets the same gate `bug update` has

Issue #674, security review `.agent/sdd/security-674.md` (medium findings 1 and 2, low
finding 3). The decision above is unchanged; this amends the fan-out's failure behavior and
adds the missing confirmation gate.

**A `comment_private` failure now stops the fan-out.** The two-call design (`Bug.add_attachment`
posts the comment public, a follow-up `Bug.update` flips it private) means the dominant flip
failure is a 403 for a credential lacking `insider`/`editbugs` — a property of the credential,
not of the bug, so it recurs on every remaining target. Continuing past it, as the original
decision allowed, republishes the user's private comment text on every bug still to come. On the
first `comment_private` failure, `upload_batch` now stops issuing new uploads; every bug it never
reached gets a `failed[]` entry with `step: "not_attempted"` naming the bug whose flip failed. An
ordinary upload failure (the non-privacy `Err` arm) still does not stop the loop — it carries no
such confidentiality cost, so the original partial-fan-out reasoning still applies to it. The
bug whose own flip failed keeps its `step: "comment_private"` entry and stays in `uploaded`,
unchanged. `succeeded + failed` still equals the bug count. `SCHEMA_VERSION` moves `3.0.4` →
`3.0.5`: `not_attempted` is a new enum value on an already-published field, additive under ADR
0007.

**`attachment upload` now calls the same `confirm_batch` gate as `bug update`.** Before this
amendment the command had neither of the repo's two batch-mutation safety gates, despite being
the less recoverable of the two: `bug update` is correctable by a second `bug update`, while
Bugzilla has no attachment-delete call. `upload_batch` now calls `confirm_batch(bug_ids.len(),
ctx.assume_yes(), w)` before the first upload, reusing the existing `BATCH_THRESHOLD` (10) and
`-y`/`--yes` bypass; a decline prints `Aborted; no changes made.` and exits 0. `--dry-run` support is
not added — `attachment upload` stays a `CommandCapabilities::authenticated` command, and
`confirm_batch` moved from `bug/update/execute.rs` into `runtime::interaction::confirm` so both
commands can share it.

**A bare `failed[]` entry was never proof the attachment was not created.** A read timeout or
dropped connection after the POST was accepted, or a 2xx response whose body carried no id, both
surface as an ordinary `Err` with no `step`, indistinguishable in the result from an upload that
never reached the server. A caller that retries a bare failure risks creating a second,
undeletable attachment on that bug. This was always true and is documentary only: the schema's
`step` description now says so, and names the remedy — list the bug's attachments before
retrying.
