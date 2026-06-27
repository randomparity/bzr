# 12. Compound `bug create` reports partial failure; it does not roll back

Status: Accepted

## Context

Issue #458 asks for a "compound/transactional" `bug create` that files a bug
together with its first comment and attachments in one invocation. The word
"transactional" suggests all-or-nothing: if a sub-step fails, undo the bug.

Two facts constrain the design:

- **Bugzilla REST has no multi-resource transaction**, and no safe, generally
  available bug-delete. `Bug.create`, `Bug.add_comment`, and
  `Bug.add_attachment` are independent POSTs. There is no server primitive to
  atomically commit or roll back the set.
- **The repo already settled this trade-off twice.** `bug clone`
  (`clone.rs`) and `attachment upload`'s comment-privacy flip (`upload.rs`) both
  comment that destructive rollback is worse than leaving the created artifact
  and surfacing it: "destructive rollback is worse than a public comment the
  user can re-target."

The issue body and every acceptance criterion describe report-and-continue, not
rollback: "print the created bug ID with a warning to stderr (never swallow it),
exit with `BatchPartialFailure` (exit 11)". TD-006 (non-atomic clone comment) is
to be fixed by *sharing the same report path*, not by adding rollback.

## Decision

1. **Run sub-steps sequentially after a successful create; never delete the
   bug.** Order: `create_bug` → `add_comment` → `upload_attachment` per
   attachment. The bug must exist before its comment/attachments can target it.

2. **On any post-create sub-step failure: report and exit 11.** The created bug
   ID is written to stdout (the result document) and named in a stderr warning
   per failed sub-step. The command returns
   `BzrError::BatchPartialFailure { succeeded, failed }` (exit 11) via the
   existing `runtime::mutation::ensure_batch_complete` helper. The bug is left in
   place as the recovery handle.

3. **Continue past the first sub-step failure; collect all failures.** A failed
   comment does not skip the attachments. The agent gets the complete failure
   set in one run rather than discovering failures across retries.

4. **Validate inputs before the create.** Attachment files are read and comment
   bodies materialized *before* `create_bug`, so a missing-file or unreadable
   input fails as input validation (exit 7) without filing an unfinishable bug.
   Only failures of POSTs that reach the server produce the exit-11 path.

5. **`bug clone` shares the same path (TD-006).** Its "Cloned from bug #N"
   comment failure now returns `BatchPartialFailure { succeeded: 1, failed: 1 }`
   (exit 11) instead of `Ok`, with the existing warning text unchanged.

## Consequences

- The created bug ID is never lost on partial failure — the issue's core goal.
  Agents recover by reading the ID and completing the missing sub-step, never by
  re-filing.
- Exit 11 is overloaded: it already means "some elements of a batch failed"; it
  now also means "the bug was created but a sub-step failed." Both are
  partial-failure-with-a-usable-ID, so the overload is coherent. `error_type` is
  `batch_partial_failure` in JSON.
- **`bug clone`'s exit code changes from 0 to 11** when the back-reference
  comment fails. A behavior change for existing callers; justified because that
  case *is* a partial failure and silently exiting 0 is the TD-006 footgun.
  Documented in CHANGELOG.
- No new client methods; the verb composes existing resource calls. The new
  surface is CLI flags, JSON keys, and one result type.

## Considered & rejected

- **True rollback (delete the bug on sub-step failure).** No safe Bugzilla
  primitive; deleting a filed bug is more destructive than leaving it. Loses the
  audit trail and can itself fail, compounding the partial state. Contradicts the
  acceptance criteria.
- **Short-circuit on the first sub-step failure.** Hides later failures; the
  agent fixes one, retries, discovers the next. Collecting all failures in one
  run is strictly more useful.
- **Exit 0 with a warning on sub-step failure** (the pre-fix clone behavior).
  This is exactly TD-006: a scripted/agent caller that checks the exit code sees
  success and never notices the missing comment/attachment.
- **A new dedicated error variant / exit code for compound partial failure.**
  `BatchPartialFailure` already models "succeeded N, failed M, here are the
  usable IDs." A second variant would fragment the partial-failure contract that
  agents already handle.
- **Posting sub-steps inside `Bug.create`** (some Bugzilla versions accept a
  `comment` on create). Not uniformly available, does not cover attachments, and
  would split the code path by server version. Composing explicit POSTs is
  uniform and testable.
