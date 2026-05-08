# `bzr attachment download`: bulk download by bug or by attachment-id list

**Date:** 2026-05-07
**Issue:** [#167](https://github.com/randomparity/bzr/issues/167)
**Parent spec:** `docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md` (Issue L)
**Status:** approved, pending implementation plan

## 1. Summary

Extend `bzr attachment download` to download every attachment of one or
more bugs in a single command, optionally mixed with explicit
attachment IDs. The current command takes exactly one attachment ID;
saving "every attachment of bug 12345 into a directory" requires
`bzr attachment list --json | jq | xargs` plumbing today. After this
change, `bzr attachment download --bug 12345 --out-dir /tmp/att`
produces `/tmp/att/12345/<att-id>.<file_name>` files directly.

The legacy single-ID shape (`bzr attachment download 9876 --out
patch.diff`) is preserved unchanged.

## 2. Background

`bzl-attachment-get` (`reference/bzl/bzl-attachment-get:51-91`) accepts
a mix of bug IDs and attachment IDs in one call and saves every matched
attachment into per-bug subdirectories, prefixing each filename with
the attachment ID. Bzr's `attachment download` accepts a single
attachment ID with an optional `--out` path. Issue #167 is filed as
part of the bzl→bzr workflow-parity review (Issue L).

Bzr already has the supporting machinery:

- `BugzillaClient::get_attachments(bug_id)` (`src/client/attachment.rs:54`)
  lists every attachment for a bug, including the API mode dispatch
  that handles Bugzilla 5.0.x's REST private-attachment filtering
  (issue #133).
- `BugzillaClient::get_attachment(id)` and
  `BugzillaClient::download_attachment(id)` (`src/client/attachment.rs:90,130`)
  fetch a single attachment by ID, returning the `bug_id` field on the
  payload.
- The `Attachment` type (`src/types/attachment.rs`) has
  `bug_id: u64`, `file_name: String`, `is_obsolete: bool`, and
  `data: Option<String>` (base64).
- `BzrError::BatchPartialFailure { succeeded, failed }`
  (`src/error.rs:46`, exit code 11) and the `BatchResult` plumbing in
  `bug update --batch` (`src/commands/bug/update.rs:207`) are the
  precedent for multi-target operations.

## 3. Scope

### In scope

- Extend `AttachmentAction::Download` to accept repeatable positional
  attachment IDs and repeatable `--bug <BUG_ID>` flags.
- Add `--out-dir <DIR>` (default `./attachments`) for the batch shape.
- Preserve the legacy single-ID shape with `--out <PATH>` unchanged.
- Validation: `--out` and `--out-dir` mutually exclusive; `--out` only
  valid with exactly one positional ID and zero `--bug` flags; at
  least one positional ID *or* one `--bug` is required.
- Per-target failure isolation: bug-level errors (404, auth) and
  per-attachment errors (decode, write, missing data) are recorded as
  failures in the batch result; the loop continues. Top-level
  `--out-dir` write failure (EACCES on `create_dir_all(out_dir)`) is
  pre-flight checked and fails fast as `BzrError::Io` (exit 6).
- New `AttachmentBatchResult` typed payload in
  `src/output/attachment.rs`, plus a renderer for human and JSON
  output.
- Sequential downloads (one HTTP fetch and one disk write at a time).
- Includes obsolete attachments (bzl parity).
- Logging: `info!` per file written.
- Tests: sibling unit tests in `src/commands/attachment_tests.rs` and
  `src/cli/mod_tests.rs`; one happy-path integration test in
  `tests/integration.rs`; functional tests in `tests/functional/`.
- Docs: `docs/bzr-cli.md` updated with all three argument shapes;
  `CHANGELOG.md` entry under the next unreleased version.

### Out of scope

- `--include-obsolete` / obsolete-filtering flag — bzl parity is to
  download all; obsolete-filtering can be a follow-up issue.
- `--concurrency N` — sequential matches `bug update --batch` and bzl;
  follow-up if testers report bulk runs are too slow.
- `--overwrite` flag — existing single-attachment behavior silently
  overwrites; we keep that for the batch shape too.
- Resume / skip-existing semantics.
- Streaming output — the batch result is rendered after all targets
  finish, matching `bug update --batch`.
- Generalizing `BatchResult<T>` to a typed payload — local
  `AttachmentBatchResult` keeps the change isolated.

## 4. CLI surface

`src/cli/attachment.rs`, `AttachmentAction::Download`:

```rust
/// Download one or more attachments to disk.
///
/// Three argument shapes are accepted:
///
/// 1. Single attachment, free-form path:
///    bzr attachment download 9876 --out patch.diff
///
/// 2. Multiple attachment IDs (positional):
///    bzr attachment download 9876 9877 9878 [--out-dir DIR]
///
/// 3. Every attachment of one or more bugs (mixable with #2):
///    bzr attachment download --bug 12345 --bug 67890 [9876] [--out-dir DIR]
///
/// In shapes 2 and 3, files are written to
/// `<out-dir>/<bug-id>/<att-id>.<file_name>`. The attachment-ID prefix
/// avoids same-name collisions on a single bug. `--out-dir` defaults
/// to `./attachments` and is created (recursively) on demand.
///
/// `--out` is only valid for shape 1.
#[command(verbatim_doc_comment)]
Download {
    /// Attachment ID(s) to download. Optional when --bug is set.
    #[arg(value_name = "ID")]
    ids: Vec<u64>,

    /// Download every attachment for the given bug. Repeatable.
    #[arg(long = "bug", value_name = "BUG_ID")]
    bug_ids: Vec<u64>,

    /// Output file path (single-attachment shape only).
    #[arg(short = 'o', long = "out", id = "out_file",
          conflicts_with_all = ["out_dir", "bug_ids"])]
    out: Option<String>,

    /// Output directory for batch downloads.
    #[arg(long = "out-dir", default_value = "./attachments")]
    out_dir: String,
},
```

Validation rules (all `BzrError::InputValidation`, exit 7) — enforced
in `validate_action`:

- `ids` empty *and* `bug_ids` empty → "specify at least one attachment
  ID or `--bug <ID>`".
- `out.is_some()` and `ids.len() != 1` → "`--out` requires exactly one
  attachment ID".
- (The `--out` vs `--out-dir`/`--bug` conflicts are enforced at the
  clap layer via `conflicts_with_all`.)

## 5. Types

`src/output/attachment.rs` — new typed payload for the batch shape:

```rust
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct AttachmentBatchResult {
    pub out_dir: String,
    pub bug_results: Vec<BugDownloadResult>,
    pub attachment_results: Vec<AttachmentDownloadResult>,
    pub summary: BatchSummary,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BugDownloadResult {
    pub bug_id: u64,
    pub status: TargetStatus,            // ok | error
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<DownloadedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct AttachmentDownloadResult {
    pub attachment_id: u64,
    pub status: TargetStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DownloadedFile {
    pub attachment_id: u64,
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchSummary {
    pub succeeded: usize,
    pub failed: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum TargetStatus { Ok, Error }
```

The legacy single-ID shape continues to use the existing
`output::DownloadResult` (`src/output/result_types.rs:86`).

## 6. Command wiring

`src/commands/attachment.rs`, replacing the existing `Download` arm:

```text
execute()
└── match AttachmentAction::Download { ids, bug_ids, out, out_dir }:
    ├── validate_action(...)                    // rules from §4
    ├── if is_legacy_single(ids, bug_ids, out): // ids.len()==1 && bug_ids empty && (out.is_some() || …)
    │     download_single_legacy(client, ids[0], out, format)
    │   else:
    │     download_batch(client, ids, bug_ids, out_dir, format)
```

`download_single_legacy` is the existing code path, factored into its
own helper for clarity. Behavior is byte-for-byte identical to today:
`client.download_attachment(id)` → `fs::write(out.unwrap_or(filename),
bytes)` → `output::print_result(DownloadResult, ...)`.

`download_batch`:

```text
download_batch(client, ids, bug_ids, out_dir, format) -> Result<()>
├── fs::create_dir_all(&out_dir)?              // pre-flight, fail fast as Io / exit 6
├── let mut bug_results = Vec::new();
├── let mut attachment_results = Vec::new();
├── for &bug_id in bug_ids:
│     match client.get_attachments(bug_id).await {
│       Ok(atts) => {
│         let mut files = Vec::new();
│         let mut first_error: Option<String> = None;
│         for att in atts {
│           match write_one_attachment(client, &att, &out_dir).await {
│             Ok(file) => files.push(file),
│             Err(e) => {
│               if first_error.is_none() { first_error = Some(e.to_string()); }
│               // continue to next attachment — partial bug success is allowed
│             }
│           }
│         }
│         let bug_status = if first_error.is_some() { TargetStatus::Error }
│                          else { TargetStatus::Ok };
│         bug_results.push(BugDownloadResult { bug_id, status: bug_status,
│                                              files, error: first_error });
│       }
│       Err(e) => bug_results.push(BugDownloadResult {
│           bug_id, status: TargetStatus::Error, files: vec![], error: Some(e.to_string()),
│       }),
│     }
├── for &att_id in ids:
│     match client.get_attachment(att_id).await {
│       Ok(att) => match write_one_attachment_from_record(&att, &out_dir).await {
│         Ok(file) => attachment_results.push(AttachmentDownloadResult {
│           attachment_id: att_id, status: TargetStatus::Ok,
│           bug_id: Some(att.bug_id), path: Some(file.path), bytes: Some(file.bytes),
│           error: None,
│         }),
│         Err(e) => attachment_results.push(AttachmentDownloadResult {
│           attachment_id: att_id, status: TargetStatus::Error,
│           bug_id: Some(att.bug_id), path: None, bytes: None,
│           error: Some(e.to_string()),
│         }),
│       },
│       Err(e) => attachment_results.push(AttachmentDownloadResult {
│           attachment_id: att_id, status: TargetStatus::Error,
│           bug_id: None, path: None, bytes: None, error: Some(e.to_string()),
│       }),
│     }
├── let summary = compute_summary(&bug_results, &attachment_results);
├── output::print_attachment_batch(&AttachmentBatchResult{...}, format);
└── if summary.failed > 0:
       return Err(BzrError::BatchPartialFailure {
           succeeded: summary.succeeded, failed: summary.failed
       });
    Ok(())
```

`write_one_attachment(client, att, out_dir)`:

1. Resolve bytes:
   - If `att.data` is `Some(b64)`, base64-decode.
   - Else fall back to `client.download_attachment(att.id).await` (which
     re-fetches via `bug/attachment/<id>` and decodes).
2. `fs::create_dir_all(<out_dir>/<att.bug_id>)`.
3. `fs::write(<out_dir>/<att.bug_id>/<att.id>.<att.file_name>, &bytes)`.
4. `tracing::info!(att_id = att.id, path = %dest, bytes = bytes.len(),
   "downloaded attachment")`.
5. Return `DownloadedFile { attachment_id: att.id, path, bytes:
   bytes.len() }`.

The fallback in step 1 makes the bulk path robust to Bugzilla
configurations where `Bug.attachments` omits the `data` field; we never
silently drop an attachment.

The hybrid REST/XML-RPC dispatch happens transparently inside
`get_attachments` and `get_attachment` — no new API mode logic.

## 7. Error handling and exit codes

| failure | type | exit |
|---|---|---|
| no IDs and no `--bug` | `InputValidation` | 7 |
| `--out` with `--bug` or with multiple positional IDs | `InputValidation` | 7 |
| `--out` and `--out-dir` both set explicitly | `InputValidation` (clap) | 7 |
| `create_dir_all(out_dir)` fails (pre-flight) | `Io` | 6 |
| `--bug X` returns 404 / auth-denied | recorded as bug-level failure | 11 |
| HTTP transport mid-batch | recorded as target failure | 11 |
| attachment has no `data` and re-fetch fails | recorded as per-attachment failure | 11 |
| base64 decode failure | recorded as per-attachment failure | 11 |
| per-bug subdir `create_dir_all` fails | recorded as per-attachment failure | 11 |
| `fs::write` fails mid-batch | recorded as per-attachment failure | 11 |
| any failures alongside any successes | `BatchPartialFailure` | 11 |
| **all** targets fail | `BatchPartialFailure` (deliberately *not* downgraded to a more specific code — matches `bug update --batch`) | 11 |
| legacy single-ID path: any of the above | propagated unchanged | as today (2/4/5/6/8/9/10) |

**Per-attachment failure within a bug.** When a single attachment
within a bug fails (decode, write, etc.), we continue to the next
attachment rather than abandoning the bug. This matches the user's
intent for bulk mode: `bug_results[].files` reflects exactly what
landed on disk, while `bug_results[].error` captures the first error
encountered. The bug's `status` is `error` if any attachment failed,
even when others succeeded — this lets the failure surface in the
batch summary while preserving the partial-success disk state.

**Per-attachment count semantics.** `summary.succeeded` counts
*attachments* written to disk, not *targets* succeeded.
`summary.failed` counts the sum of bug-level errors and
attachment-level errors. A bug with three attachments (two written,
one failed) contributes 2 to `succeeded` and 1 to `failed`. A bug
that 404s contributes 0 to `succeeded` and 1 to `failed`. This is
how the user-facing "X succeeded, Y failed" line is most useful, and
matches what they'd count manually after the run.

**Stderr behavior:** `output::print_attachment_batch` mirrors the
existing `BatchResult` text renderer — successes and per-target
failures are printed to stderr in human format with `id: error` lines.
JSON output is emitted as a single object on stdout with the shape
in §5. No partial-success warnings; overwrite-silently semantics from
Q5 mean re-runs are idempotent.

## 8. Tests

### Sibling unit tests — `src/commands/attachment_tests.rs`

- `validate_download_args_rejects_no_ids_no_bugs` — `InputValidation`,
  exit 7.
- `validate_download_args_rejects_out_with_bug` — mutual exclusion via
  clap.
- `validate_download_args_rejects_out_with_multi_ids` —
  `--out` requires single positional.
- `validate_download_args_rejects_out_and_out_dir_both_set` — clap
  conflict.
- `download_legacy_single_unchanged_with_out` — regression: existing
  shape writes to `--out` path, prints `DownloadResult`.
- `download_legacy_single_unchanged_no_out` — regression: defaults to
  original filename in cwd.
- `download_batch_one_bug_two_attachments` — wiremock: bug with two
  attachments → both files at `<dir>/<bug>/<att>.<name>`.
- `download_batch_collision_filenames_resolved_by_att_id` — two atts
  with the same `file_name` on one bug → both written, distinguished
  by attachment-ID prefix.
- `download_batch_mixed_bug_and_attachment_ids` — `--bug X` + positional
  Y → both land in `<dir>/<bug-of-Y>/...`, attachment Y's directory
  derived from its `bug_id` field.
- `download_batch_bug_not_found_partial_failure` — one bug 404s, others
  succeed → `BatchPartialFailure`, exit 11, JSON has correct rows.
- `download_batch_obsolete_attachments_included` — obsolete atts still
  downloaded (bzl parity).
- `download_batch_empty_bug_zero_files_success` — bug with no atts →
  success row with `files: []`, no warning.
- `download_batch_top_level_out_dir_unwritable_fails_fast` — EACCES on
  `create_dir_all(out_dir)` → `Io` (exit 6), no batch loop entered.
- `download_batch_per_bug_subdir_failure_recorded_as_target_failure` —
  mid-batch failure isolation.
- `download_batch_overwrites_existing_file_silently` — regression:
  re-run idempotent.
- `download_batch_json_output_shape` — snapshot against the JSON
  shape from §5.
- `download_batch_attachment_data_missing_falls_back_to_get` — when
  `Bug.attachments` returns `data: None`, the bulk path re-fetches
  via `download_attachment(id)` rather than silently skipping.

### Sibling unit tests — `src/cli/mod_tests.rs`

- `download_parses_single_id` — positional shape parses.
- `download_parses_multiple_ids` — `Vec<u64>` accepts >1.
- `download_parses_bug_flag_repeatable` — `--bug 1 --bug 2` parses.
- `download_parses_mixed_bug_and_ids` — `--bug 1 9876` parses.
- `download_parses_out_dir_default` — defaults to `./attachments`.
- `download_clap_conflict_out_with_out_dir_explicit` — clap rejects
  `--out X --out-dir Y`.
- `download_clap_conflict_out_with_bug` — clap rejects `--out X --bug 1`.

### Integration test — `tests/integration.rs`

One happy-path bulk download against a wiremock-served Bugzilla,
asserting on-disk file layout (`<out-dir>/<bug-id>/<att-id>.<file_name>`
exists with expected bytes).

### Functional tests — `tests/functional/`

Extend `run-tests.sh` with the test plan from issue #167:

```bash
# bug with two attachments
bzr attachment download --bug "$BUG_ID" --out-dir "/tmp/att_$$"
[[ -f "/tmp/att_$$/$BUG_ID/${ATT1_ID}.file1" ]]
[[ -f "/tmp/att_$$/$BUG_ID/${ATT2_ID}.file2" ]]

# mixed shape
bzr attachment download --bug "$BUG_ID" "$POSITIONAL_ATT_ID" \
    --out-dir "/tmp/att_mix_$$"
```

## 9. Documentation

- `docs/bzr-cli.md` — rewrite the `attachment download` section to
  cover all three shapes with examples.
- `CHANGELOG.md` — entry under the next unreleased version, section
  "Added": "`bzr attachment download` accepts multiple attachment IDs
  and `--bug <ID>` (repeatable) for bulk downloads into per-bug
  subdirectories (#167)."

## 10. Open questions

None blocking. Two non-blocking implementation notes:

- Whether `Bug.attachments` returns `data` by default depends on
  Bugzilla version and the `exclude_fields` query parameter. The
  fallback in `write_one_attachment` covers either behavior; no
  design change needed.
- Wiremock fixtures may need to be extended to include obsolete
  attachments and missing-`data` responses to cover the new test
  cases. This is fixture-level work, not design.
