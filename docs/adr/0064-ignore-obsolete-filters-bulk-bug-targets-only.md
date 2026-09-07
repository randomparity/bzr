# 0064 — `--ignore-obsolete` filters bulk bug targets only

- Status: Accepted
- Date: 2026-09-07
- Issue: #674
- Related: [0063](0063-multi-bug-attachment-upload-publishes-a-batch-result.md)

## Context

python-bugzilla ties `--ignore-obsolete` to `--getall <BUGID>`: the filter
applies to the set the tool enumerates for a bug, and the option has no meaning
without one (`bugzilla/_cli.py`).

`bzr attachment download` has two target streams that merge into one result:
`--bug <ID>` targets, resolved to every attachment on the bug in
`download_bug_target`, and positional attachment IDs, fetched individually in
`download_attachment_target` (`src/commands/attachment/download.rs`). Both may
appear in one invocation. Bugzilla already reports `is_obsolete` on every
attachment record and `bzr` already carries it
(`Attachment::is_obsolete`, `src/types/attachment.rs`), so nothing has to be
fetched to decide the question — only applied.

Where the filter applies is therefore a real choice, and so is what to do when
the flag arrives with nothing for it to filter.

## Decision

**`--ignore-obsolete` skips obsolete attachments on `--bug <ID>` targets only,
and an invocation carrying it with no `--bug` target is rejected with exit 7.**

- The filter is applied in `download_bug_target`, between the listing call and
  the write loop. A positional attachment ID is downloaded whether or not it is
  obsolete, because naming an ID is an explicit request for that attachment.
  This is the operator-approved exclusion E1 recorded on issue #674.
- A mixed `--bug 12345 9876 --ignore-obsolete` invocation filters the `--bug`
  leg and leaves `9876` alone, even when `9876` belongs to `12345` and is
  obsolete.
- A bug whose attachments are all obsolete is a **success with zero files** —
  `TargetStatus::Ok`, empty `files`, no error — which is what the command
  already does for a bug with no attachments at all. The filter removing
  everything is the filter working.
- Rejection is `BzrError::input` raised from `validate_action`, alongside the
  command's existing semantic guards, naming the flag and what it needs.

## Consequences

- **Skipped attachments are not counted in the result.** They are recorded at
  `tracing::debug!` and nowhere else, so `AttachmentBatchResult` and its table
  trailer keep their present shape and `--ignore-obsolete` changes no output
  contract. A caller wanting the count re-runs without the flag and subtracts,
  or reads `attachment list`.
- **Exit 7 becomes reachable on a new input.** `docs/bzr-cli.md` lists the
  command's exit-7 conditions explicitly and gains this one.
- **`attachment list` and `attachment view` gain no obsolete filter.** They
  report `is_obsolete` and callers filter downstream. This is operator-approved
  exclusion E2 on issue #674.
- **A future `--only-obsolete` or `--obsolete-only` would fit the same seam**
  without moving this decision, since the filter is one predicate in one
  function.

## Alternatives considered

- **Also filter positional attachment IDs.** verified: excluded by the
  operator-approved exclusion set frozen in the `WORK:SCOPE` annotation on issue
  #674 (exclusion E1, owner epic #665 / operator). judgment: it is also wrong on
  the merits — an explicit ID is the caller stating exactly which attachment
  they want, and silently declining it makes the ID argument conditional on a
  flag that was aimed at a different stream.
- **Accept the flag with no `--bug` target and silently ignore it.** judgment:
  it leaves `bzr attachment download 9876 --ignore-obsolete` writing the
  obsolete file the caller asked to skip, with nothing on stderr and exit 0 —
  the outcome the flag exists to prevent, reported as success.
- **Enforce the requirement with clap's `requires = "bug_ids"`.** verified: clap
  reports an unmet `requires` as a usage error with exit code 2, while every
  sibling semantic guard for this command lives in `validate_action` and returns
  `BzrError::input` — exit 7 — and `docs/bzr-cli.md` documents 7 as this
  command's input-validation code. One line of clap would split the command's
  input errors across two exit codes.
- **Report a `skipped` count on each `BugDownloadResult`.** judgment: it changes
  the bulk output shape, and its table trailer, to carry a number a debug log
  already carries for the one caller who asked to skip them.
