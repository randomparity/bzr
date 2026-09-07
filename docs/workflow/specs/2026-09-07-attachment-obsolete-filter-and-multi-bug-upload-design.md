# Attachment obsolete filter and multi-bug upload — design

- Issue: [#674](https://github.com/randomparity/bzr/issues/674)
- Epic: [#665](https://github.com/randomparity/bzr/issues/665) (decomposition entry 12)
- ADRs: [0063](../../adr/0063-multi-bug-attachment-upload-publishes-a-batch-result.md),
  [0064](../../adr/0064-ignore-obsolete-filters-bulk-bug-targets-only.md)
- Date: 2026-09-07

## Problem

Two python-bugzilla attachment capabilities have no `bzr` equivalent, and the
comparison suite records both as expected gaps against this issue:

1. `bugzilla attach --getall <BUGID> --ignore-obsolete` skips attachments the
   server marks obsolete. `bzr attachment download --bug <ID>` downloads every
   attachment for a bug and filters nothing — there is no `is_obsolete` test
   anywhere in `src/commands/attachment/download.rs`, even though the field is
   already carried on `Attachment`.
2. `bugzilla attach ID1 ID2 -f FILE` uploads one file to several bugs.
   `bzr attachment upload` takes one bug (`UploadArgs.bug_id: u64`).

Both gaps are asserted today as *controlled parser failures* in
`tests/functional/compare/03-attachments.sh`: the tests require `bzr` to exit 2
with an exact clap diagnostic, then call `expect_gap 674`. `expect_gap` converts
that FAIL into a GAP, and converts a PASS into a FAIL reading
`expected gap issue #674 appears resolved` (`tests/functional/lib.sh`). Closing
the gap therefore *must* rewrite those two tests and remove their markers, or
the comparison suite goes red on success.

## Goals

- `bzr attachment download --bug <ID>... --ignore-obsolete` skips obsolete
  attachments on the `--bug` stream.
- `bzr attachment upload <BUG_ID>... <FILE>` uploads one file to each named bug.
- Both comparison tests assert persisted-state parity with python-bugzilla and
  carry no `expect_gap` marker; both parity-report rows read `parity`.
- The harness self-tests that pin the gap rows, the gap count, and the rendered
  gap owner are moved to the parity outcome, with controlled-red coverage for
  the new assertions.

## Non-goals

Frozen as operator-approved exclusions on the issue's `WORK:SCOPE` annotation:

- **E1** `--ignore-obsolete` does not filter positional attachment-ID targets.
- **E2** No obsolete filter on `attachment list` or `attachment view`.
- **E3** No other attachment verb gains multi-bug targeting.
- **E4** python-bugzilla's `--getall` / `--get` option spellings are not adopted;
  `bzr` keeps its `--bug` / positional-ID shape.

`-f/--file` is **not** an operator-approved exclusion. It is a live alternative
that ADR 0063 considers and rejects on its own stated grounds — compatibility
with the documented file-after-bug spelling — and it remains reconsiderable on
those grounds.

Also out of scope, and recorded rather than fixed: `AttachmentBatchResult`
(the `attachment download` bulk shape) has no published schema. ADR 0063 states
why this change does not close that.

## Design

### Download — the obsolete filter

`AttachmentAction::Download` gains `ignore_obsolete: bool`
(`--ignore-obsolete`), threaded through `DownloadArgs` to `download_bug_target`.
The filter is one predicate applied to the listing result before the write loop:
an attachment with `is_obsolete == Some(true)` is skipped and logged at
`tracing::debug!`. `is_obsolete == None` is treated as not-obsolete, matching how
`src/output/resources/attachment.rs` already renders an absent value.

`validate_action` gains one guard: `--ignore-obsolete` with no `--bug <ID>`
target is `BzrError::input` (exit 7). ADR 0064 records why the guard lives there
rather than in clap, and why positional IDs are unfiltered.

A bug whose attachments are all obsolete yields `TargetStatus::Ok` with empty
`files` — the same record the command already produces for a bug with no
attachments. `AttachmentBatchResult` is otherwise unchanged, so the flag alters
no output contract.

### Upload — the fan-out

`UploadArgs.bug_id: u64` becomes
`bug_ids: Vec<u64>` with `#[arg(value_name = "BUG_ID", required = true, num_args = 1..)]`,
declared before the existing `file` positional. clap `=4.6.6` accepts a variadic
positional followed by a required one and renders
`Usage: bzr attachment upload [OPTIONS] <BUG_ID>... <FILE>`; this was confirmed
with a standalone probe binary built against that exact version.

Two argument-grammar consequences follow and are recorded in ADR 0063: an option
may no longer sit between the bug IDs and the file, and with an all-numeric
argument list the last token is always taken as the file, so an omitted file
drops the last bug target instead of failing. Both are pinned by parser tests.

`validate_action` gains a duplicate-ID guard (exit 7, naming the repeated ID).

`prepare_upload` is unchanged apart from no longer setting `params.bug_id`; the
file is read and MIME-guessed **once**, and the single owned
`UploadAttachmentParams` is re-targeted per bug by assigning `bug_id` at the top
of each iteration — no clone, so the file body is held once. Base64 encoding is
not done here at all: `UploadAttachmentParams.data` is `Vec<u8>` with
`#[serde(serialize_with = "serialize_data_as_base64")]`, so each request encodes
at serialization time.

The result shape splits on target count, per ADR 0063:

- **One bug** — today's `UploadResult` and today's text line, byte-identical.
- **Two or more** — a new `BatchUploadResult`:

  ```json
  {
    "resource": "attachment",
    "action": "created",
    "size": 1234,
    "uploaded": [{"bug_id": 12345, "attachment_id": 9876}],
    "failed": [{"bug_id": 67890, "error": "...", "step": "comment_private"}]
  }
  ```

  `step` is optional and, when present, means the attachment was created (the
  bug also appears in `uploaded`) and only the `--comment-private` follow-up
  failed. Any `failed` entry returns `BatchPartialFailure` (exit 11) through
  `ensure_batch_complete`, whose two counts are **target counts**: each bug
  contributes at most one `failed` entry, and duplicate IDs are rejected before
  the loop, so `failed` is the failed-target count, `succeeded` is
  `bug_ids.len() - failed`, and the two always sum to the number of bugs. A bug
  that received its attachment but failed the privacy flip counts **once, on the
  failed side** — it is listed in `uploaded` because the caller needs its
  `attachment_id`, but it did not fully succeed. A per-bug upload failure does not
  abort the loop and does not roll back earlier uploads.

  Table mode branches on `step` for the same reason: a `comment_private` entry
  renders as the attachment having landed with the privacy flip unapplied, never
  as `Failed to upload`, because the natural response to the latter is a retry
  that leaves an undeletable second attachment. The batch path also suppresses
  `flip_new_comment_private`'s own `warn_partial` stderr pair, so one sub-step
  failure produces one message instead of three.

The type is published as `schemas/attachment-upload-batch-result.json` and
registered in `SCHEMAS`, which makes `SCHEMA_VERSION` move `3.0.3` → `3.0.4`
(additive, per ADR 0007).

### Error handling

| Condition | Behaviour |
|---|---|
| `--ignore-obsolete` with no `--bug` | exit 7, `BzrError::input` |
| repeated bug ID on upload | exit 7, `BzrError::input`, names the ID |
| unreadable upload file | unchanged — exit 7 before any request |
| one bug of many refuses the upload | recorded in `failed`, loop continues, exit 11 |
| one bug of many fails the privacy flip | `uploaded` **and** `failed` with `step`, exit 11 |
| single-bug upload failure | unchanged — the underlying error's own exit code |
| every attachment on a `--bug` target obsolete | success, zero files |

## Validation

Every contract below is machine-checkable and gets a focused test. The plan
carries the per-task inventory; this is the coverage map.

- **Unit (`src/cli/attachment_tests.rs`)** — parser: `<BUG_ID>... <FILE>` with
  one and two IDs; `--summary` before the bug IDs and after the file (both
  accepted) and interleaved between them (`UnknownArgument`); the one-token
  `upload 12345` (`MissingRequiredArgument`); the all-numeric `upload 1 2`
  parsing as one bug and a file named `2`; a non-numeric bug ID
  (`ValueValidation`); and `--ignore-obsolete` parsing on `download`.
- **Unit (`src/commands/attachment/mod_tests.rs`)** — `validate_action`: the
  `--ignore-obsolete`-without-`--bug` rejection, the duplicate-bug-ID rejection,
  and the accepting cases for each.
- **Unit (`src/commands/attachment/download_tests.rs`)** — the filter predicate:
  obsolete skipped, `None` kept, all-obsolete yields an `Ok` record with zero
  files, positional IDs unfiltered.
- **Unit (`src/commands/attachment/upload_tests.rs`)** — the fan-out against
  `wiremock`: two bugs both succeed; one succeeds and one fails (exit 11, both
  recorded); a privacy-flip failure produces a `step`-marked entry that also
  appears in `uploaded`, renders its own table line rather than a
  `Failed to upload` line, and emits exactly one stderr message; one bug still
  emits `UploadResult`.
- **Unit (`src/commands/schema_tests.rs`)** — `assert_conforms` over a
  maximally-populated `BatchUploadResult` for the top-level contract, **plus**
  `schema_accepts` over the same value and a rejected negative case.
  `assert_conforms` compares top-level keys only and never recurses into
  `uploaded[].items` or `failed[].items`, so it cannot see the bug-to-attachment
  pairing the new type exists for; `schema_accepts` walks nested items and is
  what binds `bug_id`/`attachment_id` and the `step` enum to the schema file.
- **Functional (`tests/functional/phases/16-attachments.sh`)** — against a real
  container: multi-bug upload lands the file on both bugs (read back with
  `attachment list`); `--ignore-obsolete` omits an attachment marked obsolete by
  `attachment update --obsolete`; `--ignore-obsolete` without `--bug` exits 7;
  a repeated bug ID exits 7. Credentialless path: `attachment download` is
  `CommandCapabilities::anonymous()`, so the **download** is repeated through a
  keyless named server alias, following `tests/functional/phases/08-bugs.sh`'s
  `credentialless-named-bug-view`. The obsolete marking stays under credentials —
  `attachment update` is authenticated — so only the read arm runs anonymously.
- **Comparison (`tests/functional/compare/03-attachments.sh`)** — both tests
  rewritten from `attachment_parser_gap` to persisted-state parity, with the
  `expect_gap 674` markers removed. The phase is order-coupled and the rewrite
  must preserve it: `multi-bug-upload` runs first and is what puts a second
  attachment on the shared bugs, which is why `ignore-obsolete`'s
  `.files | length == 1` holds. The two legs share `$_ATTACH_MULTI_BUG`, so the
  bzr leg must not assert on a summary string the python leg also writes.
- **Harness (`tests/functional/pybz/container-tests.sh`)** — the two parity-report
  row strings, the `5/0/2` pass/fail/gap counts, and `attachment_assert_gap_owners`
  are moved to the parity outcome, and the fixture `run_bzr` learns to answer the
  two new commands. Controlled-red coverage is added for each new assertion.
- **Docs (`agent-skills/tests/flag-drift-check.sh`, run by `make skills-test`)** —
  the new `--ignore-obsolete` flag must appear in the `attachment download` block
  of the `## Command Tree` in `docs/bzr-cli.md` or the check fails.
