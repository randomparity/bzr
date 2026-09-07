# Attachment obsolete filter and multi-bug upload — implementation plan

**Goal.** Give `bzr attachment download` an `--ignore-obsolete` filter for its
`--bug` targets and let `bzr attachment upload` take several bug IDs, then flip
both `#674` comparison rows from expected-gap to parity.

**Architecture.** `bzr` is a layered Rust CLI: `src/cli/` holds clap derive
structs, `src/commands/<resource>/` holds one async handler per action,
`src/output/` holds result types and writers, `schemas/` holds the published
JSON contracts. Both changes stay inside `src/cli/attachment.rs` and
`src/commands/attachment/`, plus one new published result type in
`src/output/result_types.rs`. No client, transport, auth, or config code moves.

**Tech stack.** Rust 2021, `clap =4.6.6` (derive), `serde`, `tokio` (tests are
`#[tokio::test]`), `wiremock` for HTTP mocking, `tabled` for tables, `bash` for
the functional and comparison harnesses.

Expected implementation size: 900–1300 changed lines (L) — derived from the file
map and task list below: ~350 lines of `src/` production code and new schema,
~450 lines of Rust unit tests across five sibling test files, ~250 lines of bash
across the functional phase, the comparison phase, and the harness self-tests,
and ~60 lines of documentation.

## Global Constraints

Transcribed from the spec and the repository instruction files; every task's
requirements implicitly include this section.

- **Never invoke bare `cargo test`.** Use `make test-one T=<substring>` while
  iterating, `make test-fast` for `--lib` only, and `make test` before a commit.
  `make test` takes roughly three minutes and exceeds the default two-minute
  tool timeout — run it in the background and read its exit status.
- **No inline `mod tests { ... }` in `src/`.** Unit tests live in a sibling
  `<name>_tests.rs` linked by `#[cfg(test)] #[path = "<name>_tests.rs>"] mod tests;`.
  `make check-test-layout` enforces this. Every sibling file used here already
  exists; add cases to it rather than creating a new one.
- **User-facing output goes through `Writers`** (`w.out` / `w.err`) and the
  `src/output/` helpers, never `println!` / `eprintln!`. Discard the result with
  `let _ = writeln!(…)` where the function does not already allow `?`.
- **Clippy pedantic with `-D warnings`.** `unwrap_used` is denied in `src/`;
  `expect_used` and `allow_attributes` warn. Test siblings open with the
  file-level `#![expect(clippy::unwrap_used)]` they already carry.
- **Lib-only clippy denies dead code.** A new type cannot be committed before
  its only caller. Task 2 therefore lands the result type, its schema, and the
  fan-out that produces it in one commit.
- **`SCHEMA_VERSION` is `3.0.3` today** (`src/output/mod.rs`) and becomes
  `3.0.4` in Task 2. It is pinned by hand in ten other files; Task 2 lists every
  one.
- **clap version floor and ceiling are the same pin:** `clap = { version = "=4.6.6" }`
  in `Cargo.toml`. Do not change it.
- **Conventional commits**, imperative, ≤72-character subject. At least one
  commit must carry `feat(attachment): …` so the change reaches the generated
  release notes; infra scopes (`test`, `docs`, `ci`, …) are excluded by design.
- **Functional tests are mandatory** for a user-facing change (Task 3), and a
  full `make functional-test` run must be green before the PR opens.
- **Shell scripts under `tests/functional/`** are shellcheck'd and `bash -n`'d by
  `make check-shell`; phase and compare scripts use four-space indentation and are
  not `shfmt`-formatted.
- **Guardrails:** `make lint` (fmt, clippy, six repo guards), `make test`,
  `make skills-test` (runs the flag-drift check against `docs/bzr-cli.md`),
  `make functional-test`, `make functional-compare`.

---

## Task 1 — `attachment download --ignore-obsolete`

**Creates:** nothing.
**Modifies:** `src/cli/attachment.rs`, `src/commands/attachment/mod.rs`,
`src/commands/attachment/download.rs`.
**Tests:** `src/cli/attachment_tests.rs`, `src/cli/mod_tests.rs`,
`src/commands/attachment/mod_tests.rs`,
`src/commands/attachment/download_tests.rs`.

**Where this fits.** First and independent: it touches no upload code and no
published schema, so it can land, be reviewed, and be reverted on its own.

### Interfaces

Consumes from the existing codebase (each confirmed present at the path named):

- `AttachmentAction::Download { ids: Vec<u64>, bug_ids: Vec<u64>, out: Option<String>, out_dir: String }`
  — `src/cli/attachment.rs`.
- `fn validate_action(action: &AttachmentAction) -> Result<()>` —
  `src/commands/attachment/mod.rs`.
- `pub(super) struct DownloadArgs<'a> { ids: &'a [u64], bug_ids: &'a [u64], out: Option<&'a str>, out_dir: &'a str }`
  — `src/commands/attachment/download.rs`.
- `async fn download_bug_target(client: &BugzillaClient, bug_id: u64, out_dir: &str) -> BugDownloadResult`
  — `src/commands/attachment/download.rs`.
- `pub is_obsolete: Option<bool>` on `crate::types::attachment::Attachment` —
  `src/types/attachment.rs`.
- `BzrError::input(String) -> BzrError` — `src/error.rs`, exit code 7.

Provides to later tasks: nothing. Task 3 exercises the flag from bash; Task 5
documents it.

### Verification

- **Contract: `--ignore-obsolete` parses on `attachment download`.**
  Mode: focused-test. Observable: the parsed `AttachmentAction::Download` carries
  `ignore_obsolete: true`. Test: `src/cli/attachment_tests.rs`, new case
  `download_parses_ignore_obsolete`. Expected red before the field exists:
  compile error `struct AttachmentAction::Download has no field named ignore_obsolete`.
  Green: `make test-one T=download_parses_ignore_obsolete`.
- **Contract: `--ignore-obsolete` without `--bug` is rejected with exit 7.**
  Mode: focused-test. Observable: `validate_action` returns
  `Err(BzrError::InputValidation(_))` whose `exit_code()` is 7. Test:
  `src/commands/attachment/mod_tests.rs`, new case
  `download_ignore_obsolete_without_bug_is_rejected`. Expected red: the call
  returns `Ok(())`. Green: `make test-one T=download_ignore_obsolete_without_bug`.
- **Contract: obsolete attachments are skipped on a `--bug` target.**
  Mode: focused-test. Observable: with a `wiremock` bug listing of two
  attachments, one `is_obsolete: true`, the resulting `BugDownloadResult.files`
  has length 1 and names the non-obsolete attachment. Test:
  `src/commands/attachment/download_tests.rs`, new case
  `download_bug_ignore_obsolete_skips_obsolete_attachments`. Expected red: length
  2. Green: `make test-one T=download_bug_ignore_obsolete`.
- **Contract: positional attachment IDs stay unfiltered.**
  Mode: focused-test. Observable: `download --bug B 9876 --ignore-obsolete` where
  `9876` is obsolete still writes `9876`. Test:
  `src/commands/attachment/download_tests.rs`, new case
  `download_ignore_obsolete_leaves_positional_ids_alone`. Expected red: the file
  is absent. Green: `make test-one T=download_ignore_obsolete_leaves_positional`.
- **Contract: a bug whose attachments are all obsolete succeeds with zero files.**
  Mode: focused-test. Observable: `execute` returns `Ok(())` and the
  `AttachmentBatchResult` JSON has `bug_results[0].status == "ok"` with an empty
  `files`. Test: `src/commands/attachment/download_tests.rs`, new case
  `download_bug_all_obsolete_succeeds_with_no_files`. Expected red: the command
  returns `Err(BatchPartialFailure)` or writes a file. Green:
  `make test-one T=download_bug_all_obsolete`.

### Steps

1. In `src/cli/attachment.rs`, inside the `Download { … }` variant and after the
   `bug_ids` field, add:

   ```rust
        /// Skip attachments the server marks obsolete. Applies to
        /// `--bug <ID>` targets only; an attachment named by its own
        /// positional ID is always downloaded.
        #[arg(long = "ignore-obsolete")]
        ignore_obsolete: bool,
   ```

2. In the same file's `Download` doc comment, add two lines to the `Examples:`
   block, before the `See bzr-attachment-list(1)` line:

   ```
   ///   bzr attachment download --bug 12345 --ignore-obsolete
   ///   bzr attachment download --bug 12345 --bug 67890 --ignore-obsolete --out-dir /tmp/live
   ```

3. In `src/commands/attachment/mod.rs`, extend the `AttachmentAction::Download`
   arm of `execute` to destructure and forward the new field:

   ```rust
        AttachmentAction::Download {
            ids,
            bug_ids,
            out,
            out_dir,
            ignore_obsolete,
        } => {
            download::handle(
                download::DownloadArgs {
                    ids,
                    bug_ids,
                    out: out.as_deref(),
                    out_dir,
                    ignore_obsolete: *ignore_obsolete,
                },
                ctx,
                format,
                w,
            )
            .await?;
        }
   ```

4. In the same file's `validate_action`, add one arm immediately after the
   existing `ids.is_empty() && bug_ids.is_empty()` arm:

   ```rust
        AttachmentAction::Download {
            bug_ids,
            ignore_obsolete: true,
            ..
        } if bug_ids.is_empty() => Err(crate::error::BzrError::input(
            "--ignore-obsolete filters bulk bug targets; add --bug <ID>".into(),
        )),
   ```

5. In `src/commands/attachment/download.rs`, add the field to `DownloadArgs`:

   ```rust
   pub(super) struct DownloadArgs<'a> {
       pub(super) ids: &'a [u64],
       pub(super) bug_ids: &'a [u64],
       pub(super) out: Option<&'a str>,
       pub(super) out_dir: &'a str,
       pub(super) ignore_obsolete: bool,
   }
   ```

   and add the same field to the private `BatchTargets<'a>` struct below it,
   setting it from `args.ignore_obsolete` where `BatchTargets` is built in
   `handle`.

6. In the same file, thread it into the per-bug walk in `download_batch`:

   ```rust
       for &bug_id in targets.bug_ids {
           bug_results.push(
               download_bug_target(client, bug_id, targets.out_dir, targets.ignore_obsolete).await,
           );
       }
   ```

7. In the same file, widen `download_bug_target`'s signature and filter the
   listing. Replace the `let mut files = Vec::new();` line and the `for att in
   &atts` header with:

   ```rust
   async fn download_bug_target(
       client: &BugzillaClient,
       bug_id: u64,
       out_dir: &str,
       ignore_obsolete: bool,
   ) -> BugDownloadResult {
       // … unchanged listing call …
       let mut files = Vec::new();
       let mut first_error: Option<String> = None;
       for att in atts.iter().filter(|att| !skip_obsolete(att, ignore_obsolete)) {
   ```

   and add the predicate beside it:

   ```rust
   /// True when `--ignore-obsolete` is set and the server marked this
   /// attachment obsolete. An absent `is_obsolete` is treated as
   /// not-obsolete, matching how the attachment table renders it.
   fn skip_obsolete(att: &Attachment, ignore_obsolete: bool) -> bool {
       let skipped = ignore_obsolete && att.is_obsolete.unwrap_or(false);
       if skipped {
           tracing::debug!(att_id = att.id, "skipping obsolete attachment");
       }
       skipped
   }
   ```

8. Add `ignore_obsolete: false` to every existing `AttachmentAction::Download {`
   construction site so the crate compiles. Find them with
   `rg -n 'AttachmentAction::Download \{' src/` — the sites are in
   `src/cli/attachment_tests.rs`, `src/cli/mod_tests.rs`, and
   `src/commands/attachment/download_tests.rs`. Sites that already destructure
   with `..` need no change.

9. Write the five test cases named in the Verification inventory. Model each on
   its nearest neighbour in the same file: the parser cases on
   `download_parses_bug_ids` in `src/cli/attachment_tests.rs`, the validation
   case on the existing `download_without_ids_or_bug_is_rejected` in
   `src/commands/attachment/mod_tests.rs`, and the three behavioural cases on the
   existing bulk-download `wiremock` cases in
   `src/commands/attachment/download_tests.rs` (which already mount
   `GET /rest/bug/<id>/attachment` and assert on the emitted JSON).

10. Confirm each new test fails before its production change and passes after.
    Run `make test-one T=ignore_obsolete` and expect five passing tests and no
    failures.

11. Run `make lint`. Expect no output from `cargo fmt`, no clippy warnings, and
    each of the six guard scripts printing its own success line.

12. Run `make test` in the background and read its exit status. Expect `0` and a
    summary line per suite with no failure block.

13. Commit: `feat(attachment): skip obsolete attachments on bulk download`.

### Acceptance criteria

- `bzr attachment download --bug <ID> --ignore-obsolete` writes only
  non-obsolete attachments for that bug.
- `bzr attachment download 9876 --ignore-obsolete` exits 7 with the message in
  step 4.
- `bzr attachment download --bug <ID> 9876 --ignore-obsolete` writes `9876` even
  when it is obsolete.
- A bug with only obsolete attachments exits 0 with zero files written.
- `make lint` and `make test` are green.

### Rollback

Single commit, no persisted state, no schema. `git revert` is sufficient.

---

## Task 2 — multi-bug `attachment upload`

**Creates:** `schemas/attachment-upload-batch-result.json`.
**Modifies:** `src/cli/attachment.rs`, `src/commands/attachment/mod.rs`,
`src/commands/attachment/upload.rs`, `src/output/result_types.rs`,
`src/output/mod.rs`, `src/commands/schema.rs`, and the ten files pinning
`SCHEMA_VERSION` listed in step 8.
**Tests:** `src/cli/attachment_tests.rs`, `src/cli/mod_tests.rs`,
`src/commands/attachment/mod_tests.rs`,
`src/commands/attachment/upload_tests.rs`, `src/commands/schema_tests.rs`.

**Where this fits.** Second, and one commit: the repository's lib-only clippy
denies dead code, so `BatchUploadResult` cannot be committed before the fan-out
that constructs it.

### Interfaces

Consumes from the existing codebase (each confirmed present at the path named):

- `pub(crate) struct UploadArgs { pub bug_id: u64, pub file: String, … }` —
  `src/cli/attachment.rs`. `bug_id` is the field this task replaces.
- `fn prepare_upload(args: &crate::cli::UploadArgs) -> Result<PreparedUpload>` and
  `struct PreparedUpload { params: UploadAttachmentParams, size: usize, comment_private: bool }`
  — `src/commands/attachment/upload.rs`.
- `async fn flip_new_comment_private(client: &BugzillaClient, bug_id: u64, new_attachment_id: u64, w: &mut Writers<'_>) -> Result<()>`
  — `src/commands/attachment/upload.rs`.
- `pub bug_id: u64` on `crate::types::attachment::UploadAttachmentParams` —
  `src/types/attachment.rs`.
- `async fn BugzillaClient::upload_attachment(&self, params: &UploadAttachmentParams) -> Result<u64>`
  — `src/client/resources/attachment.rs`.
- `pub fn write_result<W: Write + ?Sized>(value: &(impl Serialize + ?Sized), human_message: &str, format: OutputFormat, out: &mut W)`
  — `src/output/result_types.rs`.
- `pub(crate) fn ensure_batch_complete(succeeded: usize, failed: usize) -> Result<()>`
  — `src/commands/runtime/mutation.rs`.
- `pub enum ResourceKind` (variant `Attachment`) and `pub enum ActionKind`
  (variant `Created`) — `src/output/result_types.rs`.
- `pub(crate) const SCHEMAS: &[(&str, &str)]`, built by the `schema_registry!`
  macro from bare names — `src/commands/schema.rs`.
- `fn assert_conforms(name: &str, value: &Value)` — `src/commands/schema_tests.rs`.

Provides to later tasks:

- CLI shape `bzr attachment upload <BUG_ID>... <FILE>` (Tasks 3, 4, 5).
- Published schema name `attachment-upload-batch-result` (Task 5's schema list).
- `SCHEMA_VERSION` value `3.0.4` (Task 3's envelope phase already pins it).

### Verification

- **Contract: the positional arity accepts one or more bug IDs before the file.**
  Mode: focused-test. Observable: parsing `["bzr","attachment","upload","1","2","f.txt"]`
  yields `bug_ids == vec![1, 2]` and `file == "f.txt"`; parsing with one ID yields
  `bug_ids == vec![1]`. Test: `src/cli/attachment_tests.rs`, new cases
  `upload_parses_single_bug_id` and `upload_parses_multiple_bug_ids`. Expected
  red before the widening: compile error `no field bug_ids on UploadArgs`. Green:
  `make test-one T=upload_parses`.
- **Contract: the one-token form is a usage error.** Mode: focused-test.
  Observable: `try_parse_from(["bzr","attachment","upload","12345"])` returns
  `ErrorKind::MissingRequiredArgument`. Test: `src/cli/attachment_tests.rs`, new
  case `upload_without_file_is_a_usage_error`. Expected red: `Ok(_)`. Green:
  `make test-one T=upload_without_file`. Pin this exact input; the multi-token
  case below behaves differently and the two must not be conflated.
- **Contract: an all-numeric argument list takes its last token as the file.**
  Mode: focused-test. Observable:
  `try_parse_from(["bzr","attachment","upload","1","2"])` yields
  `bug_ids == vec![1]` and `file == "2"` — an omitted file silently drops the last
  bug target rather than failing, per ADR 0063's second consequence. Test:
  `src/cli/attachment_tests.rs`, new case
  `upload_all_numeric_args_take_the_last_as_the_file`. Expected red: an
  `ErrorKind` is returned. Green: `make test-one T=upload_all_numeric_args`. The
  test exists to make the ambiguity a recorded contract, not to endorse it.
- **Contract: an option between the bug IDs and the file is rejected.** Mode:
  focused-test. Observable:
  `try_parse_from(["bzr","attachment","upload","1","--summary","s","f.txt"])`
  returns `ErrorKind::UnknownArgument`, while both
  `["…","1","f.txt","--summary","s"]` and `["…","--summary","s","1","f.txt"]`
  parse. Test: `src/cli/attachment_tests.rs`, new case
  `upload_rejects_an_option_between_bug_ids_and_file`. Expected red: the
  interleaved form parses. Green:
  `make test-one T=upload_rejects_an_option_between`. This pins ADR 0063's first
  consequence — a real break in a previously valid spelling — so it cannot
  regress silently in either direction.
- **Contract: a non-numeric bug ID is a value error.** Mode: focused-test.
  Observable: `try_parse_from(["bzr","attachment","upload","1","x","f.txt"])`
  returns `ErrorKind::ValueValidation`. Test: `src/cli/attachment_tests.rs`, new
  case `upload_rejects_a_non_numeric_bug_id`. Expected red: `Ok(_)`. Green:
  `make test-one T=upload_rejects_a_non_numeric`.
- **Contract: a repeated bug ID is rejected with exit 7.** Mode: focused-test.
  Observable: `validate_action` returns `Err` whose `exit_code()` is 7 and whose
  message contains the repeated ID. Test:
  `src/commands/attachment/mod_tests.rs`, new case
  `upload_duplicate_bug_ids_are_rejected`. Expected red: `Ok(())`. Green:
  `make test-one T=upload_duplicate_bug_ids`.
- **Contract: one bug still emits `UploadResult`.** Mode: focused-test.
  Observable: the captured stdout parses as an object with a scalar `bug_id` and
  no `uploaded` key. Test: `src/commands/attachment/upload_tests.rs`, new case
  `upload_single_bug_keeps_the_upload_result_shape`. Expected red: an `uploaded`
  array is present. Green: `make test-one T=upload_single_bug_keeps`.
- **Contract: two bugs both receive the file and are paired in the result.**
  Mode: focused-test. Observable: two `wiremock` mounts, one per bug; the
  captured stdout has `uploaded == [{bug_id:1,attachment_id:11},{bug_id:2,attachment_id:22}]`
  and an empty `failed`; `execute` returns `Ok(())`. Test:
  `src/commands/attachment/upload_tests.rs`, new case
  `upload_fans_out_to_every_bug`. Expected red: only one request is made. Green:
  `make test-one T=upload_fans_out`.
- **Contract: a per-bug failure is recorded and exits 11.** Mode: focused-test.
  Observable: bug 1 mounts a success, bug 2 mounts a Bugzilla error envelope;
  `execute` returns `Err(BzrError::BatchPartialFailure { succeeded: 1, failed: 1 })`
  whose `exit_code()` is 11, and the captured stdout lists bug 1 under `uploaded`
  and bug 2 under `failed`. Test: `src/commands/attachment/upload_tests.rs`, new
  case `upload_partial_failure_records_both_outcomes`. Expected red: the first
  failure aborts the loop. Green: `make test-one T=upload_partial_failure`.
- **Contract: a privacy-flip failure is a `step`-marked entry that also appears
  in `uploaded`, and exits 11 with the right counts.** Mode: focused-test.
  Observable: with `--comment` and `--comment-private` over two bugs, bug 2's
  comment listing mounts an error; the result has bug 2 in `uploaded` **and** in
  `failed` with `step == "comment_private"`, and the returned error is
  `BatchPartialFailure { succeeded: 1, failed: 1 }` — bug 2 received the
  attachment but did not fully succeed, so it counts once, on the failed side,
  and the two counts sum to the number of bugs. Test:
  `src/commands/attachment/upload_tests.rs`, new case
  `upload_comment_private_failure_is_a_sub_step`. Expected red: `Ok(())` is
  returned, or bug 2 is absent from `uploaded`. Green:
  `make test-one T=upload_comment_private_failure`.
- **Contract: the count invariant holds when every sub-step fails.** Mode:
  focused-test. Observable: with `--comment-private` over two bugs whose flips
  both fail, `succeeded + failed == bug_ids.len()`. Test:
  `src/commands/attachment/upload_tests.rs`, new case
  `upload_batch_invariant_holds_when_every_flip_fails`. Expected red: the counts
  do not sum to the target count. Green:
  `make test-one T=upload_batch_invariant`.
- **Contract: table mode distinguishes a sub-step failure from an upload failure
  and does not double-narrate it.** Mode: focused-test. Observable: the same
  two-bug privacy-flip scenario under `OutputFormat::Table` writes one stderr line
  for bug 2 naming the attachment as uploaded with the privacy flip unapplied,
  contains no `Failed to upload to bug #2`, and does not repeat
  `warn_partial`'s two lines. Test: `src/commands/attachment/upload_tests.rs`,
  new case `upload_batch_table_mode_labels_a_sub_step_failure`. Expected red:
  `Failed to upload to bug #2` appears, or three stderr lines do. Green:
  `make test-one T=upload_batch_table_mode`.
- **Contract: the published schema matches the serialized type, nested shapes
  included.** Mode: focused-test. Observable: for a maximally-populated
  `BatchUploadResult` carrying one plain `failed` entry and one with
  `step: Some("comment_private")`, both
  `assert_conforms("attachment-upload-batch-result", &value)` and
  `assert!(schema_accepts("attachment-upload-batch-result", &value))` pass, and a
  negative sample — an `uploaded[]` entry missing `attachment_id` — is rejected by
  `schema_accepts`. Test: `src/commands/schema_tests.rs`, new case
  `attachment_upload_batch_result_conforms`, modelled on the `comment` cases that
  already pair the two helpers. Expected red before the schema file exists:
  `no schema registered for attachment-upload-batch-result` from `schema_for`.
  Green: `make test-one T=attachment_upload_batch_result_conforms`.
  **`assert_conforms` alone is not enough here** — it compares top-level keys
  only and never recurses into `uploaded[].items` or `failed[].items`, so it
  cannot see the bug-to-attachment pairing this type exists for, nor the `step`
  enum. `schema_accepts` (`src/commands/schema_tests.rs`) walks nested items and
  is what binds them.

### Steps

1. In `src/cli/attachment.rs`, replace the `bug_id` field of `UploadArgs` with:

   ```rust
       /// Bug ID(s) to attach the file to. Repeatable; the file is
       /// uploaded once to each bug. A repeated ID is rejected.
       #[arg(value_name = "BUG_ID", required = true, num_args = 1..)]
       pub bug_ids: Vec<u64>,
   ```

   Leave the `file` field immediately after it, unchanged. clap `=4.6.6` accepts
   a variadic positional followed by a required one; the rendered usage becomes
   `Usage: bzr attachment upload [OPTIONS] <BUG_ID>... <FILE>`.

2. In the same file's `Upload` doc comment, change the opening sentence to name
   the fan-out and add one example line:

   ```
   /// Reads the local file at `<file>` and uploads it as an
   /// attachment on every bug in `<bug_id>...`.
   ```
   ```
   ///   bzr attachment upload 12345 67890 patch.diff
   ```

3. In `src/output/result_types.rs`, add the batch types after `UploadResult`:

   ```rust
   /// One bug that received the uploaded file, paired with the attachment
   /// the server created on it.
   #[derive(Debug, Serialize)]
   #[non_exhaustive]
   pub struct UploadTarget {
       pub bug_id: u64,
       pub attachment_id: u64,
   }

   /// One bug that did not fully receive the upload. `step` is present only
   /// when the attachment itself was created — the bug then also appears in
   /// [`BatchUploadResult::uploaded`] — and only the follow-up
   /// `--comment-private` flip failed. A caller must not retry a
   /// `comment_private`-stepped failure with the same `--comment` text:
   /// `Bug.add_attachment` posts a new comment on every call.
   #[derive(Debug, Serialize)]
   #[non_exhaustive]
   pub struct UploadFailure {
       pub bug_id: u64,
       pub error: String,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub step: Option<String>,
   }

   impl UploadFailure {
       pub fn new(bug_id: u64, error: impl Into<String>) -> Self {
           Self { bug_id, error: error.into(), step: None }
       }

       pub fn comment_private(bug_id: u64, error: impl Into<String>) -> Self {
           Self { bug_id, error: error.into(), step: Some("comment_private".into()) }
       }
   }

   /// Result of one `attachment upload` fanned out over two or more bugs.
   /// A single-bug upload emits [`UploadResult`] instead, so this body
   /// appears only for a multi-bug invocation.
   #[derive(Debug, Serialize)]
   #[non_exhaustive]
   pub struct BatchUploadResult {
       pub resource: ResourceKind,
       pub action: ActionKind,
       pub size: usize,
       pub uploaded: Vec<UploadTarget>,
       pub failed: Vec<UploadFailure>,
   }

   impl BatchUploadResult {
       #[must_use]
       pub fn new(size: usize, uploaded: Vec<UploadTarget>, failed: Vec<UploadFailure>) -> Self {
           Self {
               resource: ResourceKind::Attachment,
               action: ActionKind::Created,
               size,
               uploaded,
               failed,
           }
       }
   }
   ```

4. Create `schemas/attachment-upload-batch-result.json`:

   ```json
   {
     "$schema": "https://json-schema.org/draft/2020-12/schema",
     "$id": "https://github.com/randomparity/bzr/schemas/attachment-upload-batch-result.json",
     "title": "BatchUploadResult",
     "description": "Result of one `attachment upload` fanned out over two or more bugs. A single-bug upload emits `upload-result` instead. Any `failed[]` entry means exit 11; a `failed[]` entry carrying `step: \"comment_private\"` means the attachment was created (the bug is also listed in `uploaded`) and only the follow-up comment-privacy flip failed.",
     "type": "object",
     "properties": {
       "resource": {
         "type": "string",
         "enum": ["bug", "attachment", "comment", "user", "group", "product", "component", "server"]
       },
       "action": {
         "type": "string",
         "enum": ["created", "updated", "added", "removed", "renamed", "downloaded", "dry-run"]
       },
       "size": {
         "type": "integer",
         "minimum": 0,
         "description": "Byte length of the uploaded file. Stated once: the same file is sent to every bug."
       },
       "uploaded": {
         "type": "array",
         "items": {
           "type": "object",
           "properties": {
             "bug_id": { "type": "integer", "minimum": 0 },
             "attachment_id": { "type": "integer", "minimum": 0 }
           },
           "required": ["bug_id", "attachment_id"],
           "additionalProperties": false
         }
       },
       "failed": {
         "type": "array",
         "items": {
           "type": "object",
           "properties": {
             "bug_id": { "type": "integer", "minimum": 0 },
             "error": { "type": "string" },
             "step": {
               "description": "Present when the attachment was created and only the `--comment-private` follow-up failed; absent when the upload itself failed. Do not retry a `comment_private` failure with the same `--comment` text.",
               "type": "string",
               "enum": ["comment_private"]
             }
           },
           "required": ["bug_id", "error"],
           "additionalProperties": false
         }
       }
     },
     "required": ["resource", "action", "size", "uploaded", "failed"],
     "additionalProperties": false
   }
   ```

5. In `src/commands/schema.rs`, add `"attachment-upload-batch-result",` to the
   `schema_registry!` list, immediately after `"attachment",` so the list stays
   sorted.

6. Rewrite `src/commands/attachment/upload.rs`'s `handle` and `prepare_upload`:

   ```rust
   pub(super) async fn handle(
       args: &crate::cli::UploadArgs,
       ctx: &CommandContext,
       format: OutputFormat,
       w: &mut Writers<'_>,
   ) -> Result<()> {
       let mut prepared = prepare_upload(args)?;
       let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
       if let [bug_id] = args.bug_ids[..] {
           return upload_single(&client, &mut prepared, bug_id, format, w).await;
       }
       upload_batch(&client, &mut prepared, &args.bug_ids, format, w).await
   }

   /// One bug: the original shape, byte for byte. Kept separate from the
   /// batch path so the common case's result contract cannot drift.
   ///
   /// `prepared` is borrowed mutably and re-targeted in place rather than
   /// cloned: `UploadAttachmentParams.data` is the whole file as `Vec<u8>`
   /// (base64 is applied by `serialize_data_as_base64` at request time), so a
   /// clone per bug would hold the file body once per target for no reason.
   async fn upload_single(
       client: &BugzillaClient,
       prepared: &mut PreparedUpload,
       bug_id: u64,
       format: OutputFormat,
       w: &mut Writers<'_>,
   ) -> Result<()> {
       prepared.params.bug_id = bug_id;
       let att_id = client.upload_attachment(&prepared.params).await?;
       if prepared.comment_private {
           flip_new_comment_private(client, bug_id, att_id, w).await?;
       }
       write_result(
           &UploadResult::new(att_id, bug_id, prepared.size),
           &format!(
               "Uploaded attachment #{att_id} to bug #{bug_id} ({} bytes)",
               prepared.size,
           ),
           format,
           w.out,
       );
       Ok(())
   }

   /// Two or more bugs: upload the same prepared payload to each, recording
   /// per-bug outcomes and continuing past a failure. A bug whose upload
   /// succeeded but whose `--comment-private` flip failed appears in both
   /// `uploaded` and `failed`.
   async fn upload_batch(
       client: &BugzillaClient,
       prepared: &mut PreparedUpload,
       bug_ids: &[u64],
       format: OutputFormat,
       w: &mut Writers<'_>,
   ) -> Result<()> {
       let mut uploaded = Vec::new();
       let mut failed = Vec::new();
       for &bug_id in bug_ids {
           prepared.params.bug_id = bug_id;
           match client.upload_attachment(&prepared.params).await {
               Ok(att_id) => {
                   uploaded.push(UploadTarget { bug_id, attachment_id: att_id });
                   if prepared.comment_private {
                       // `flip_new_comment_private` narrates its own failure to
                       // stderr for the single-bug path. On the batch path the
                       // result body and the table renderer already report it,
                       // so the quiet variant is used and one failure produces
                       // one message.
                       if let Err(e) = flip_new_comment_private_quiet(client, bug_id, att_id).await
                       {
                           failed.push(UploadFailure::comment_private(bug_id, e.to_string()));
                       }
                   }
               }
               Err(e) => failed.push(UploadFailure::new(bug_id, e.to_string())),
           }
       }
       let result = BatchUploadResult::new(prepared.size, uploaded, failed);
       write_batch_upload(&result, format, w);
       // Count targets, not array lengths. Each bug contributes at most one
       // `failed` entry — an upload failure or a sub-step failure, never both —
       // and duplicate IDs were rejected before the loop, so the array length is
       // the failed-target count and the two counts sum to the number of bugs.
       // Subtracting from `bug_ids.len()` is what prevents the double-count; a
       // bug in both `uploaded` and `failed` still counts once, on the failed
       // side, because it did not fully succeed.
       let failed_targets = result.failed.len();
       ensure_batch_complete(bug_ids.len() - failed_targets, failed_targets)
   }

   /// Render a [`BatchUploadResult`]. JSON and NDJSON emit the object; the
   /// table form prints one line per upload and sends each failure to stderr.
   /// A `comment_private` entry is **not** rendered as an upload failure: the
   /// attachment exists, and telling the operator the upload failed invites a
   /// retry that leaves a second attachment Bugzilla cannot delete.
   fn write_batch_upload(result: &BatchUploadResult, format: OutputFormat, w: &mut Writers<'_>) {
       match format {
           OutputFormat::Json | OutputFormat::Ndjson => write_result(result, "", format, w.out),
           OutputFormat::Table => {
               for t in &result.uploaded {
                   let _ = writeln!(
                       w.out,
                       "Uploaded attachment #{} to bug #{} ({} bytes)",
                       t.attachment_id, t.bug_id, result.size,
                   );
               }
               for f in &result.failed {
                   if f.step.is_some() {
                       let _ = writeln!(
                           w.err,
                           "Uploaded to bug #{} but could not make the comment private: {}",
                           f.bug_id, f.error,
                       );
                   } else {
                       let _ =
                           writeln!(w.err, "Failed to upload to bug #{}: {}", f.bug_id, f.error);
                   }
               }
           }
       }
   }
   ```

   Split `flip_new_comment_private` so the batch path can stay quiet. Keep the
   existing function as the single-bug entry point — signature and `warn_partial`
   behaviour unchanged — and move its body into
   `async fn flip_new_comment_private_quiet(client: &BugzillaClient, bug_id: u64, new_attachment_id: u64) -> Result<()>`,
   which takes no `Writers` and does not call `warn_partial`. The existing
   function becomes:

   ```rust
   async fn flip_new_comment_private(
       client: &BugzillaClient,
       bug_id: u64,
       new_attachment_id: u64,
       w: &mut Writers<'_>,
   ) -> Result<()> {
       flip_new_comment_private_quiet(client, bug_id, new_attachment_id)
           .await
           .inspect_err(|e| warn_partial(new_attachment_id, e, w))
   }
   ```

   and the three `.inspect_err(|e| warn_partial(...))` calls inside the moved
   body are dropped, since the wrapper now warns once for the single-bug path.

   In `prepare_upload`, change the destructuring binding from `bug_id` to
   `bug_ids`, drop the `params.bug_id = *bug_id;` line, and prefix the unused
   binding with an underscore (`bug_ids: _`), since the caller owns the targets.
   Add `use crate::output::result_types::{BatchUploadResult, UploadFailure, UploadTarget};`
   and `use crate::commands::runtime::mutation::ensure_batch_complete;` to the
   file's imports.

   `UploadAttachmentParams` needs no new derive: the loop re-targets one owned
   value in place rather than cloning it, so its existing
   `#[derive(Debug, Serialize)]` (`src/types/attachment.rs`) stands unchanged.

7. In `src/commands/attachment/mod.rs`, add the duplicate guard as a statement
   ahead of `validate_action`'s `match`, rather than as a match arm — a guard
   cannot bind the duplicate it found, and calling the finder twice to recover it
   is the shape that invites a `.unwrap()` the lint denies:

   ```rust
   fn validate_action(action: &AttachmentAction) -> Result<()> {
       if let AttachmentAction::Upload(crate::cli::UploadArgs { bug_ids, .. }) = action {
           if let Some(dup) = first_duplicate(bug_ids) {
               return Err(crate::error::BzrError::input(format!(
                   "bug #{dup} is listed more than once; each bug may be named once",
               )));
           }
       }
       match action {
           // … the existing arms, unchanged …
       }
   }
   ```

   and add the helper beside `update_has_changes`:

   ```rust
   /// The first bug ID that appears twice in `ids`, if any. `ids` is a
   /// hand-typed command line, so the quadratic scan is bounded by what a
   /// person types and avoids pulling in a set for a handful of numbers.
   fn first_duplicate(ids: &[u64]) -> Option<u64> {
       ids.iter()
           .enumerate()
           .find(|&(i, id)| ids[..i].contains(id))
           .map(|(_, id)| *id)
   }
   ```

   The predicate pattern is `&(i, id)`, with exactly one `&`: `Iterator::find`
   hands the closure `&Self::Item`, and `Self::Item` here is `(usize, &u64)`, so
   a bare `(i, id)` binds `id` as `&&u64` while `slice::contains` wants `&u64`,
   and a doubled `&&(i, id)` over-dereferences and fails with `E0308`. `find`
   itself returns `Option<(usize, &u64)>`, which is why the following `map`
   dereferences once.

8. Bump `SCHEMA_VERSION` from `"3.0.3"` to `"3.0.4"` in `src/output/mod.rs`, then
   update every hand-written pin. Find them with
   `rg -n '3\.0\.3' --glob '!target/**' --glob '!docs/workflow/**' .` and change
   each occurrence that states the current envelope version (not a narrated
   history entry) in:

   - `README.md`
   - `docs/bzr-cli.md`
   - `content/skills/bzr-reference/reference/commands.md`
   - `content/skills/bzr-reference/reference/json-recipes.md`
   - `content/skills/bzr-dependency-analysis/scripts/collect.py`
   - `content/skills/bzr-dependency-analysis/tests/test_collect.py`
   - `content/skills/bzr-dependency-analysis/tests/fixtures/recording_runner.py`
   - `tests/functional/phases/18a-json-envelope.sh`
   - `tests/functional/phases/18c-skills-install.sh`
   - `tests/functional/phases/18d-dependency-analysis.sh`
   - `tests/functional/phases/08e-bugs-restricted-access.sh`

   Leave `Cargo.lock` (an unrelated crate version) and
   `docs/adr/0062-field-list-enumerates-the-accepted-write-field-set.md` (a
   historical statement) alone.

9. Update every `UploadArgs {` construction site from `bug_id: N` to
   `bug_ids: vec![N]`. Find them with `rg -n 'UploadArgs *\{' src/` — the sites
   are in `src/cli/attachment.rs` (the definition), `src/cli/attachment_tests.rs`,
   `src/cli/mod_tests.rs`, `src/commands/attachment/mod.rs` (a pattern, which
   uses `..`), `src/commands/attachment/mod_tests.rs`, and
   `src/commands/attachment/upload_tests.rs`.

10. Write the twelve test cases named in the Verification inventory. For the
    schema case, model it on the `comment` cases in
    `src/commands/schema_tests.rs`, which already pair `assert_conforms` with
    `schema_accepts` and a rejected negative sample. Populate every top-level
    field so `assert_conforms`'s top-level bijection holds — it compares
    `value.as_object().keys()` against `schema["properties"].keys()` in both
    directions and stops there. `step` lives at `failed[].items.properties.step`,
    which `assert_conforms` never reads, so it is `schema_accepts` that must carry
    the `step` entry and the `uploaded[]` pairing.

11. Confirm each new test fails before its production change and passes after.
    Run `make test-one T=upload` and `make test-one T=attachment_upload_batch_result`
    and expect all of them passing with no failures.

12. Regenerate nothing — the schema is hand-written and embedded by
    `include_str!` at build time. Run `cargo build` and expect it to succeed;
    a missing schema file is a compile error naming the path.

13. Run `make lint`, then `make test` in the background. Expect exit `0` from
    both.

14. Commit: `feat(attachment): upload one file to several bugs`.

### Acceptance criteria

- `bzr attachment upload 1 2 f.txt` uploads `f.txt` to bugs 1 and 2 and emits a
  `BatchUploadResult` pairing each bug with its new attachment.
- `bzr attachment upload 1 f.txt` emits exactly the `UploadResult` object and the
  same text line it emits today.
- `bzr attachment upload 1 1 f.txt` exits 7 naming bug 1.
- A per-bug failure exits 11 with both outcomes in the result and no rollback.
- `bzr schema attachment-upload-batch-result` prints the new schema and
  `bzr schema` lists it.
- `bzr --json whoami` reports `"schema_version": "3.0.4"`.
- `make lint` and `make test` are green.

### Rollback

Single commit. Reverting it restores `SCHEMA_VERSION` `3.0.3` and removes the
schema together with its registry entry, so no consumer is left pointing at a
schema that no longer exists.

---

## Task 3 — functional phase coverage

**Creates:** nothing.
**Modifies:** `tests/functional/phases/16-attachments.sh`.
**Tests:** the phase script is the test.

**Where this fits.** Third: it exercises Tasks 1 and 2 against a real Bugzilla
container, which is the only tier that catches a REST response-shape mismatch.

### Interfaces

Consumes from the existing harness (each confirmed present in
`tests/functional/lib.sh` or the phase file):

- `test_begin <id> <description>`, `test_pass`, `test_fail <reason>`,
  `test_skip <reason>` — `tests/functional/lib.sh`.
- `run_bzr <args…>`, which captures stdout to `$BZR_STDOUT`, stderr to
  `$BZR_STDERR`, and the status to `$BZR_EXIT` — `tests/functional/lib.sh`.
- `assert_success`, `assert_exit_code <n>`, `assert_json <jq-path> <value>` —
  `tests/functional/lib.sh`.
- `make_bug --product FuncTestProd --component Backend --op-sys Linux --platform PC --description d --summary <S>`
  — used throughout `tests/functional/phases/16-attachments.sh`.
- `$BUG1` and `$ATTACH_ID`, set earlier in the same phase file.
- `$BZ_URL` — `http://127.0.0.1:${BZ_PORT}`, set in `tests/functional/run-tests.sh`
  and in scope for every phase; used by the credentialless alias in step 6.
- The keyless named-alias pattern
  `run_bzr config set-server <name> --url "$BZ_URL" --api rest` … `--server <name>` …
  `run_bzr config remove-server <name>`, imported from
  `tests/functional/phases/08-bugs.sh` (`credentialless-named-bug-view`).

Provides to later tasks: nothing.

### Verification

- **Contract: multi-bug upload lands the file on every named bug.**
  Mode: focused-test. Observable: after
  `attachment upload $A $B file`, `attachment list $A` and `attachment list $B`
  each report an attachment with the summary used. Test:
  `tests/functional/phases/16-attachments.sh`, new test ID
  `attachment-upload-fans-out-to-several-bugs`. Expected red before Task 2:
  exit 2 with `unexpected argument`. Green: `make functional-test`.
- **Contract: `--ignore-obsolete` omits an obsolete attachment.**
  Mode: focused-test. Observable: with two attachments on one bug and one marked
  obsolete via `attachment update <ID> --obsolete`, the `--ignore-obsolete` bulk
  download writes exactly one file into the per-bug subdirectory. Test: same
  file, new test ID `attachment-download-ignore-obsolete-skips-obsolete`.
  Expected red before Task 1: exit 2. Green: `make functional-test`.
- **Contract: `--ignore-obsolete` without `--bug` exits 7.**
  Mode: focused-test. Observable: `$BZR_EXIT` is 7. Test: same file, new test ID
  `attachment-download-ignore-obsolete-requires-bug`. Expected red: exit 2.
  Green: `make functional-test`.
- **Contract: a repeated bug ID on upload exits 7.**
  Mode: focused-test. Observable: `$BZR_EXIT` is 7. Test: same file, new test ID
  `attachment-upload-rejects-a-repeated-bug-id`. Expected red: exit 0 with two
  attachments on the bug. Green: `make functional-test`.
- **Contract: the obsolete filter works without credentials.**
  Mode: focused-test. Observable: the same bulk download, re-run through a keyless
  named server alias, writes the same single file. `attachment download` is
  `CommandCapabilities::anonymous()` (`src/commands/attachment/mod.rs`), so only
  the **download** runs credentialless; the `attachment update --obsolete` setup
  is an authenticated command and stays on the credentialed path from the previous
  test. Test: same file, new test ID
  `attachment-download-ignore-obsolete-anonymous`. Expected red: exit 2. Green:
  `make functional-test`.

### Steps

1. Open `tests/functional/phases/16-attachments.sh` and read the two existing
   bulk-download tests (`attachment-download-bug-bulk-into-per-bug-subdir` and
   `attachment-download-mixes-bug-and-positional-ids`). The new tests go
   immediately after them and follow the same shape: guard on the fixture
   variables, `mktemp -d`, `run_bzr`, `assert_success`, count files with
   `find … | wc -l | tr -d ' '`, then `rm -rf` the directory.

2. Add `attachment-upload-fans-out-to-several-bugs`. Create a second bug with
   `make_bug`, upload one temp file to both with
   `run_bzr attachment upload "$BUG1" "$_FANOUT_BUG" "$_FANOUT_FILE" --summary "fanout"`,
   assert success, then for each bug run `run_bzr attachment list <ID>` and
   require `jq -e '[.[] | select(.summary == "fanout")] | length == 1'` on
   `"$BZR_STDOUT"` to succeed. `run_bzr` always passes `--json` and
   `_project_envelope` has already unwrapped `.data`, so the jq path starts at
   the payload. Fail with the bug ID that came up short.

3. Add `attachment-upload-rejects-a-repeated-bug-id`:
   `run_bzr attachment upload "$BUG1" "$BUG1" "$_FANOUT_FILE"` then
   `assert_exit_code 7`.

4. Add `attachment-download-ignore-obsolete-skips-obsolete`. Create a bug, upload
   two files to it, mark the first obsolete with
   `run_bzr attachment update <ID> --obsolete`, then
   `run_bzr attachment download --bug <BUG> --ignore-obsolete --out-dir "$_OBS_DIR"`
   and require exactly one file under `"$_OBS_DIR/<BUG>"`. Assert also that the
   obsolete attachment's ID does **not** appear in the written filenames, since
   the harness writes `<att-id>.<file_name>`.

5. Add `attachment-download-ignore-obsolete-requires-bug`:
   `run_bzr attachment download "$ATTACH_ID" --ignore-obsolete --out-dir "$_OBS_DIR"`
   then `assert_exit_code 7`.

6. Add `attachment-download-ignore-obsolete-anonymous`. Phase 16 has **no**
   credentialless invocation to copy — every `run_bzr` in it passes
   `--server-api-key-env` / `--server-email` — so import the pattern from
   `tests/functional/phases/08-bugs.sh` (`credentialless-named-bug-view`) and the
   named-alias precedent already in this file at its
   `production-shaped-attachment-wire-variants` block:

   ```bash
   run_bzr config set-server obsolete-anon --url "$BZ_URL" --api rest
   ```

   then re-run **only** the download from step 4 as
   `run_bzr --server obsolete-anon attachment download --bug <BUG> --ignore-obsolete --out-dir "$_OBS_ANON_DIR"`,
   require the same single file, and clean up with
   `run_bzr config remove-server obsolete-anon` whether or not the assertion
   passed. Do not repeat step 4's `attachment update --obsolete` here: it is an
   authenticated command and would fail without a key. `BZ_URL` is set by
   `tests/functional/run-tests.sh` and is in scope for every phase.

7. Run `make check-shell`. Expect shellcheck and `bash -n` to print nothing for
   the phase file; four-space indentation is required and `shfmt` does not run
   over phase scripts.

8. Run `make check-functional-test-ids`. Expect it to accept the five new IDs;
   it rejects an ID that is not unique or not referenced from the runner.

9. Run `make functional-test` (roughly ten minutes; run it in the background and
   read its exit status). Expect exit `0` and the five new IDs reported `PASS`.

10. Commit: `test(functional): cover obsolete filter and multi-bug upload`.

### Acceptance criteria

- Five new test IDs appear in `make functional-test` output, all `PASS`.
- `make check-shell` and `make check-functional-test-ids` are green.
- No existing test ID changed.

### Rollback

Single commit touching one test file. `git revert` is sufficient.

---

## Task 4 — comparison parity and harness self-tests

**Creates:** nothing.
**Modifies:** `tests/functional/compare/03-attachments.sh`,
`tests/functional/pybz/container-tests.sh`,
`docs/dev/python-bugzilla-parity.md`.
**Tests:** `tests/functional/pybz/container-tests.sh` is the harness's own test
suite; `make functional-compare` is the integration proof.

**Where this fits.** Fourth, and the task that closes the issue: `expect_gap`
converts a passing gap test into a FAIL, so the comparison suite goes red the
moment Tasks 1 and 2 land unless this task runs.

### Interfaces

Consumes from the existing harness (each confirmed present at the path named):

- `expect_gap <issue-number>` — `tests/functional/lib.sh`. Converts the current
  test's FAIL into a GAP, and converts a PASS into a FAIL reading
  `expected gap issue #<n> appears resolved`.
- `resource_expect_gap <issue>`, `resource_gap_reset`, `resource_gap_allow` —
  `tests/functional/lib.sh`.
- `resource_bzr <name> <mode> <transport> <args…>` and
  `resource_pybz <name> <operation> <payload-json> <transport>` —
  `tests/functional/lib.sh`; the python side writes
  `$COMPARE_EXCHANGE_DIR/<name>.pybz.result.json`.
- `attachment_create_bug <name> <summary>` setting `$ATTACHMENT_CREATED_BUG_ID`,
  and the fixtures `$ATTACHMENT_PYBZ_BUG_ID`, `$ATTACHMENT_BZR_BUG_ID`,
  `$ATTACHMENT_PYBZ_ID`, `$ATTACHMENT_BZR_ID`, `$ATTACHMENT_SOURCE`,
  `$ATTACHMENT_STEM`, `$RESOURCE_SERVER` — `tests/functional/compare/03-attachments.sh`.
- python-bugzilla adapter operations `attachment_upload` (accepts a `bug_ids`
  array and returns `{"attachment_ids": [...]}`) and `attachment_cli_download_bug`
  (accepts `ignore_obsolete` and returns `{"bug_id":…,"files":[…]}`) — already
  implemented; the current gap tests call both and validate their evidence.
- `assert_equals <expected> <actual> <label>` and `run_attachment_control <flag> <slug> [count]`
  — `tests/functional/pybz/container-tests.sh`.

Provides to later tasks: the parity-report row text Task 5 must not contradict.

### Verification

- **Contract: `multi-bug-upload` asserts persisted parity and carries no marker.**
  Mode: focused-test. Observable: in the harness fixture run,
  `compare/03-attachments/multi-bug-upload` reports `PASS` and `GAP_COUNT` does
  not increase. Test: `tests/functional/pybz/container-tests.sh`, the existing
  fixture run whose counts move from `5 PASS / 0 FAIL / 2 GAP` to
  `7 PASS / 0 FAIL / 0 GAP`. Expected red before the rewrite: the count assertion
  fails with `2` gaps. Green: `bash tests/functional/pybz/container-tests.sh`
  through `make functional-compare`.
- **Contract: `ignore-obsolete` asserts persisted parity and carries no marker.**
  Mode: focused-test. Same observable and same test as above; the two tests move
  together because they share the fixture run's counts.
- **Contract: each new parity assertion can be driven red.** Mode: focused-test.
  Observable: `run_attachment_control ATTACHMENT_MULTI_PARITY_FAULT multi-bug-upload`
  and `run_attachment_control ATTACHMENT_OBSOLETE_PARITY_FAULT ignore-obsolete`
  each produce at least one FAIL naming the slug. Test:
  `tests/functional/pybz/container-tests.sh`, two new controls. Expected red if
  the assertion is vacuous: the control passes and the harness prints
  `attachment control … unexpectedly passed`. Green: the same harness run.
- **Contract: the parity report has no `#674` row left.** Mode: focused-test.
  Observable: the harness's parity-report row list contains the two new `parity`
  strings and no `expected gap (#674)` string. Test:
  `tests/functional/pybz/container-tests.sh`, the existing row-list assertion at
  its `| Multi-bug attachment upload |` and `| Ignore obsolete attachments |`
  entries. Expected red: the row list still names `#674`. Green: the same run.

### Steps

1. In `tests/functional/compare/03-attachments.sh`, delete the
   `attachment_parser_gap` helper and both of its call sites. Nothing else uses
   it; confirm with `rg -n attachment_parser_gap tests/`.

2. Rewrite `test_begin "multi-bug-upload"`. Keep the existing python-bugzilla
   leg unchanged — it creates the second bug, calls `attachment_upload` with a
   two-element `bug_ids`, and requires two positive `attachment_ids`. Replace the
   `run_bzr … ; attachment_parser_gap 674 …` tail with a persisted-state
   comparison:

   **The two legs share `$_ATTACH_MULTI_BUG`.** The python leg above already
   uploaded to `[$ATTACHMENT_PYBZ_BUG_ID, $_ATTACH_MULTI_BUG]` with
   `summary:"multi upload"`, so that bug will hold **two** attachments once the
   bzr leg runs. Give the bzr leg its own summary and assert on that, never on a
   string both tools write.

   - `resource_bzr multi-bzr-upload rest REST attachment upload "$ATTACHMENT_BZR_BUG_ID" "$_ATTACH_MULTI_BUG" "$ATTACHMENT_SOURCE" --summary "multi upload bzr"`
   - require `jq -e '.uploaded | length == 2'` on
     `"$COMPARE_EXCHANGE_DIR/multi-bzr-upload.bzr.stdout.json"`, and require the
     two `bug_id` values to equal the two bug IDs passed. That capture holds the
     projected payload — `run_bzr` always passes `--json` and `_project_envelope`
     has already unwrapped `.data` — so the jq path starts at the payload;
   - read each bug back with
     `resource_bzr multi-bzr-list-<n> rest REST attachment list <BUG>` and require
     exactly one attachment whose `summary` is `multi upload bzr`;
   - `test_pass` when both bugs carry it, `test_fail` naming the bug that does
     not.

   Remove the `resource_expect_gap 674` call.

3. Rewrite `test_begin "ignore-obsolete"`. Keep the existing setup and python leg
   — it marks `$ATTACHMENT_PYBZ_ID` obsolete, runs
   `attachment_cli_download_bug` with `ignore_obsolete: true`, and requires the
   python result to list exactly one file. Replace the tail with:

   **This test depends on `multi-bug-upload` having run first, on both sides.**
   The python leg's `.files | length == 1` holds only because `multi-bug-upload`
   put a second attachment on `$ATTACHMENT_PYBZ_BUG_ID`; step 2 does the same for
   `$ATTACHMENT_BZR_BUG_ID`. Do **not** add a further upload here — that would
   leave the bzr bug with two non-obsolete attachments against the python bug's
   one and break the symmetry the cross-check depends on. State the ordering
   dependency in a comment above the test so a later editor does not reintroduce
   it or reorder the phase.

   - mark the bzr-side attachment obsolete with
     `resource_bzr obsolete-bzr-setup rest REST attachment update "$ATTACHMENT_BZR_ID" --obsolete`;
   - `resource_bzr obsolete-bzr-download rest REST attachment download --bug "$ATTACHMENT_BZR_BUG_ID" --ignore-obsolete --out-dir "$COMPARE_EXCHANGE_DIR/obsolete-bzr"`;
   - require exactly one file under
     `"$COMPARE_EXCHANGE_DIR/obsolete-bzr/$ATTACHMENT_BZR_BUG_ID"`, and require
     its name not to begin with `$ATTACHMENT_BZR_ID.`;
   - compare that count with the python leg's `.files | length` and `test_pass`
     only when they agree.

   Remove the `resource_expect_gap 674` call.

4. In `tests/functional/pybz/container-tests.sh`, replace the two row strings in
   the parity-report list:

   ```
   '| Multi-bug attachment upload | `bzr attachment upload` | parity | `compare/03-attachments/multi-bug-upload` |'
   '| Ignore obsolete attachments | `bzr attachment download --bug --ignore-obsolete` | parity | `compare/03-attachments/ignore-obsolete` |'
   ```

5. The file has **two** bzr fixtures, and the one to edit is not the obvious one.
   `resource_bzr` (around line 2431) is the fixture with the command arms
   (`bug create`, `attachment upload`, `attachment list`, `attachment view`,
   `attachment download`, `attachment update`) and it is what the rewritten tests
   in steps 2 and 3 call. The fixture `run_bzr` (around line 2612) has no arms at
   all: it emits a clap diagnostic and exists only to feed
   `attachment_parser_gap`, which step 1 deleted.

   **Delete the fixture `run_bzr` outright**, together with its
   `ATTACHMENT_GAP_STALE` and `ATTACHMENT_GAP_WRONG_DIAGNOSTIC` branches and the
   two controls that drive them. Then extend `resource_bzr`'s arms:

   - `"attachment upload "*` — when the command carries two bug IDs, write
     `{"uploaded":[{"bug_id":A,"attachment_id":N},{"bug_id":B,"attachment_id":N+1}],"failed":[],"size":19}`
     through `attachment_fixture_write_bzr`, honouring
     `ATTACHMENT_MULTI_PARITY_FAULT` by emitting a one-element `uploaded`. Keep
     the existing single-bug `{"id":$next_bzr_attachment}` answer for one ID.
   - `"attachment list "*` — add a `multi-bzr-list-*` branch to the existing
     `case $name in` that answers with one record whose summary is
     `multi upload bzr`.
   - `"attachment download "*` — when the command carries `--ignore-obsolete`,
     write the file under the non-obsolete attachment's ID rather than the
     existing `${ATTACHMENT_BZR_ID}.attachment-source.txt` path, since step 3
     asserts that `$ATTACHMENT_BZR_ID`'s file is *absent*. Honour
     `ATTACHMENT_OBSOLETE_PARITY_FAULT` by writing the obsolete attachment's file
     as well.

6. In the same file, change the fixture run's count assertions from
   `assert_equals 5 "$PASS_COUNT"`, `assert_equals 0 "$FAIL_COUNT"`,
   `assert_equals 2 "$GAP_COUNT"` to `7`, `0`, `0`. Then remove the gap-owner
   machinery — **all four residues, not just the function**:

   - `attachment_assert_gap_owners` itself (around line 2650);
   - its direct call (around line 2670);
   - its **second** call inside the `ATTACHMENT_GAP_OWNER_FAULT` control block
     (around lines 2701–2709), and that whole block;
   - the `expect_gap` / `attachment_fixture_expect_gap` override pair (around
     lines 2404–2412), which exists only to drive that control.

   Deleting the function while leaving the control behind is silently worse than
   leaving both: `if attachment_assert_gap_owners; then` on a missing function
   makes bash return 127, the `if` reads that as "correctly failed", and the
   control reports a controlled red unconditionally from then on.

7. In the same file, add the two controls named in the Verification inventory
   beside the existing `run_attachment_control` calls:

   ```bash
   run_attachment_control ATTACHMENT_MULTI_PARITY_FAULT multi-bug-upload
   run_attachment_control ATTACHMENT_OBSOLETE_PARITY_FAULT ignore-obsolete
   ```

8. In `docs/dev/python-bugzilla-parity.md`, change the two `#674` rows to read
   `parity`, matching the strings written in step 4 exactly — the harness
   compares them literally.

9. Run `make check-shell`. Expect no shellcheck or `bash -n` output for the three
   shell files.

10. Run `make functional-compare` (longer than `make functional-test`; run it in
    the background and read its exit status). Expect exit `0`, the two tests
    reported `PASS`, and a gap count for `#674` of zero.

11. Commit: `test(compare): assert attachment parity for the closed #674 gaps`.

### Acceptance criteria

- `rg -n '674' tests/functional/` returns nothing.
- `rg -n 'ATTACHMENT_GAP_OWNER_FAULT|attachment_assert_gap_owners|ATTACHMENT_GAP_STALE|ATTACHMENT_GAP_WRONG_DIAGNOSTIC|attachment_parser_gap' tests/functional/`
  returns nothing.
- The harness fixture run reports `7 PASS / 0 FAIL / 0 GAP` for the attachment
  comparison phase.
- Both new controls drive their assertion red.
- `make functional-compare` is green.

### Rollback

Single commit. Reverting it restores both `expect_gap` markers, which then
report FAIL against the shipped capability — so this commit must not be reverted
without also reverting Tasks 1 and 2.

---

## Task 5 — CLI reference

**Creates:** nothing.
**Modifies:** `docs/bzr-cli.md`, `tests/functional/phases/18-completion-schema.sh`.
**Tests:** `agent-skills/tests/flag-drift-check.sh`, run by `make skills-test`;
`tests/functional/phases/18-completion-schema.sh`, run by `make functional-test`.

**Where this fits.** Last: it documents the surface Tasks 1 and 2 created, and
its guard needs the built binary.

### Interfaces

Consumes: the `## Command Tree` block and the `### bzr attachment download` /
`### bzr attachment upload` sections of `docs/bzr-cli.md`; the published schema
list near the end of the same file. Provides: nothing.

### Verification

- **Contract: every long flag the binary exposes appears in that command's tree
  block, and vice versa.** Mode: focused-test. Observable:
  `agent-skills/tests/flag-drift-check.sh` exits 0. Test: the script itself, run
  by `make skills-test`. Expected red before the tree edit: it reports
  `--ignore-obsolete` present in the binary and absent from the tree. Green:
  `make skills-test` (roughly ten minutes; run in the background).
- **Contract: `bzr schema` lists the new schema.** Mode: focused-test.
  Observable: `assert_schema_list_contains attachment-upload-batch-result`
  succeeds. Test: `tests/functional/phases/18-completion-schema.sh`, added to its
  existing `assert_schema_list_contains` chain. Expected red before the registry
  entry: the assertion reports the name absent. Green: `make functional-test`.
  The `docs/bzr-cli.md` schema-list paragraph is prose that no guard compares
  against the registry, so it is updated as an unguarded step below rather than
  claimed as a tested contract.

### Steps

1. In the `## Command Tree` block, change the attachment lines to:

   ```
   │   ├── download <ATTACHMENT_ID> [--bug <ID>] [--ignore-obsolete] [-o|--out <FILE>] [--out-dir <DIR>]
   │   ├── upload <BUG_ID>... <FILE> [--summary <S>] [--content-type <MIME>] [--comment <BODY>]
   ```

   Leave the continuation lines of the `upload` block unchanged.

2. In `### bzr attachment download`, add
   `bzr attachment download [<ID>...] --bug <BUG_ID>... [--ignore-obsolete] [--out-dir <DIR>]`
   to the synopsis, add the flag row

   ```
   | `--ignore-obsolete` | Skip attachments the server marks obsolete. Applies to `--bug` targets only; a positional `<ID>` is always downloaded. Requires at least one `--bug`. |
   ```

   to the flags table, add one example, and add
   `--ignore-obsolete with no --bug` to the exit-7 bullet.

3. In `### bzr attachment upload`, change the `<BUG_ID>` option row to
   `` `<BUG_ID>...` `` with the description
   `Bug ID(s). One or more, before the file. A repeated ID exits 7.`, add a
   multi-bug example, and add an **Output** subsection stating the split: one bug
   emits `UploadResult`; two or more emit `BatchUploadResult`, where a per-bug
   failure exits 11 and leaves earlier uploads in place, and a
   `step: "comment_private"` entry means the attachment landed and only the
   privacy flip failed — do not retry it.

   Add an **Argument order** note carrying the two grammar consequences from
   ADR 0063, because both are ways a previously working command line changes
   behaviour: options go before the bug IDs or after the file, never between
   (`upload 12345 --summary s patch.diff` now exits 2); and with an all-numeric
   argument list the last token is the file, so `upload 12345 67890` means bug
   `12345` and a file named `67890`.

4. Add `attachment-upload-batch-result` to the published schema list paragraph,
   beside `upload-result`, and to the `assert_schema_list_contains` chain in
   `tests/functional/phases/18-completion-schema.sh`.

5. Confirm the `3.0.4` pins Task 2 made in this file are still correct after
   these edits: `rg -n '3\.0\.[0-9]' docs/bzr-cli.md` must show only `3.0.4`.

6. Run `make skills-test` in the background and read its exit status. Expect `0`.
   Then run `make check-shell` for the phase-script edit, and re-run
   `make functional-test` in the background so the extended
   `assert_schema_list_contains` chain is exercised. Expect `0` from each.

7. Commit: `docs(cli): document the obsolete filter and multi-bug upload`.

### Acceptance criteria

- `make skills-test`, `make check-shell`, and `make functional-test` are green.
- The command tree, the two command sections, the exit-code list, the argument-
  order note, and the schema list all describe the shipped surface.
- `bzr schema` lists `attachment-upload-batch-result`, asserted by
  `tests/functional/phases/18-completion-schema.sh`.

### Rollback

Documentation only. `git revert` is sufficient.

---

## Deferrals

None carried into this plan from the design review. `AttachmentBatchResult`'s
missing published schema is recorded in ADR 0063's consequences as out-of-scope
debt, not as a deferral this change owns.
