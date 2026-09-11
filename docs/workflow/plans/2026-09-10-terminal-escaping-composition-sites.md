# Plan: terminal-control escaping at composition sites outside the writers

**Goal.** Escape Unicode `Cc` plus the Trojan-Source bidi set in the three bypassing sinks ADR
0065 left open — the batch command-layer stderr lines, the `src/main.rs` `error: {…}` table
renderings, and (by contract) the `write_result`/`write_saved` composed-string arms — by
exposing `escape_terminal_controls` at `bzr::output::escape_terminal_controls` (a `pub use`
re-export) and escaping each server-controlled interpolation
at its composition point.

**Architecture.** Widen `escape_terminal_controls` to `pub` in `src/output/formatting.rs` and
add a `pub use` re-export in `src/output/mod.rs` so `src/commands/**` and the binary reach the
existing, unchanged predicate at `bzr::output::escape_terminal_controls`. Each sink then wraps
its server-controlled interpolation in `escape_terminal_controls` at the composition point.
The `write_result`/`write_saved` table arms stay unescaped at the seam; their callers
own their interpolations, and `src/commands/schema.rs` passes static names (a no-op).

Design: `docs/workflow/specs/2026-09-10-terminal-escaping-composition-sites-design.md`.
Decision: `docs/adr/0070-escape-terminal-controls-at-composition-sites.md`.
Size band S: ~25 source lines, ~70 lines of unit cases across four `*_tests.rs` siblings, one
~60-line functional phase, ~6 lines of docs.

Expected implementation size: 120–180 changed lines (S) — derived from six small source edits
(~25 lines), ~70 lines of unit cases across four siblings, one ~60-line functional phase, and
~6 lines of docs.

## Global constraints

- Unit tests in sibling `<name>_tests.rs` (`#[cfg(test)] #[path = …] mod tests;`); inline
  `mod tests` in `src/` is forbidden (`make check-test-layout`).
- User output via `Writers` / the `output` helpers, never `println!`. Clippy pedantic,
  `-D warnings`; `unwrap_used` denied.
- Never call `colored::control::set_override` in a test — process-global, flakes parallel
  tests. Inspect `ColoredString.fgcolor` instead.
- The `--json` / `--output ndjson` family is a published schema surface: do NOT escape it. The
  dispatch-error JSON/NDJSON arms stay on `serde_json`.
- The `write_result` / `write_saved` table arms must NOT escape at the seam —
  `src/commands/schema.rs` passes `names.join("\n")` and depends on the embedded newlines.
- Guardrails `make lint`, `make test`, `make functional-test`; iterate with
  `make test-one T=<substr>` / `make test-fast`. Never bare `cargo test`.
- Phase scripts: 4-space indent, `# shellcheck shell=bash` header, gated by `make check-shell`;
  ids `^[a-z0-9]+(-[a-z0-9]+)*$`, globally unique (`tools/check-functional-test-ids.sh`).
- Solo run: add the ADR 0070 row to `docs/adr/README.md` yourself (the index is not CI-gated).
  Match neighbouring rows in length and tone; give the row's `Status` cell the same value as
  the record's `## Status` (`Accepted`).
- Do not touch `src/client/**`, `src/http.rs`, `src/xmlrpc/**`, `src/config/**`, or the
  JSON/NDJSON encoding.

## File map

| File | Answerable for |
|---|---|
| `src/output/formatting.rs` | `escape_terminal_controls` visibility `pub(super)` → `pub`; refreshed doc comment |
| `src/output/mod.rs` | `pub use formatting::escape_terminal_controls;` re-export (exposes `bzr::output::escape_terminal_controls`) |
| `src/commands/bug/update/output.rs` | escape `f.error` in the "Failed to update bug #…: …" stderr line |
| `src/commands/bug/create_json.rs` | escape `f.error` in the "Failed to create bug (item …): …" stderr line |
| `src/main.rs` | escape the `BzrError` at both `error: {…}` table renderings (resolve_format arm + dispatch Table arm) |
| `src/output/result_types.rs` | doc comment on `write_result` recording the "composed string; callers escape their own interpolations" contract |
| `src/commands/bug/update/output_tests.rs` | one case: per-item error escapes in the batch-update stderr line |
| `src/commands/bug/create_json_tests.rs` | one case: per-item error escapes in the batch-create stderr line |
| `src/main_tests.rs` | one case: dispatch-error Table arm escapes; JSON/NDJSON arms unchanged |
| `src/output/result_types_tests.rs` | one case: `write_result` table arm prints a `\n`-carrying message verbatim (seam unchanged) |
| `tests/functional/phases/08i-batch-error-escaping.sh` | end-to-end proof against a real container |
| `docs/bzr-cli.md` | the stderr rendering change |
| `docs/adr/README.md` | the ADR 0070 index row |

## Task 1 — expose the shared helper at `bzr::output::escape_terminal_controls`

Modifies `src/output/formatting.rs` and `src/output/mod.rs`.

**Interfaces.** Widen `pub(super) fn escape_terminal_controls(value: &str) -> String` in
`src/output/formatting.rs` to `pub`, and add a `pub use formatting::escape_terminal_controls;`
re-export in `src/output/mod.rs` (the `formatting` module is private, so the re-export is what
makes it reachable). The body, the predicate (`character.is_control() ||
BIDI_CONTROLS.contains(&character)`), and the `escape_default()` spelling are unchanged. Every
later task consumes it as `crate::output::escape_terminal_controls` (the command layer) or
`bzr::output::escape_terminal_controls` (the binary).

**Verification.** Contract: the predicate is unchanged and the helper is reachable from the
binary. `Mode: focused-test`. `formatting_tests.rs::escape_terminal_controls_escapes_cc_and_bidi_only`
(existing) pins the predicate; a regression here means the body was touched. Green:
`make test-one T=escape_terminal_controls_escapes_cc_and_bidi_only`. Binary reachability is a
compile-time proof: `src/main.rs` (a separate crate) can only name
`bzr::output::escape_terminal_controls` if the `pub use` exists, so Task 4 compiling is the
reachability check. The doc comment itself is `Mode: task-test-not-applicable` (prose).

1. In `src/output/formatting.rs`, change the function's visibility from `pub(super)` to `pub`
   and extend its doc comment. Keep the existing paragraphs (the predicate, the three seams,
   the JSON-family exclusion) and add one paragraph:

   ```rust
   /// ADR 0070 exposes this at `bzr::output::escape_terminal_controls` (re-exported in
   /// `src/output/mod.rs`) so the command layer (`src/commands/**`) and the binary
   /// (`src/main.rs`) can escape the server-controlled interpolations in the messages they
   /// compose outside `src/output/` — the batch stderr failure lines and the `error: {…}`
   /// renderings. Those sites escape per-interpolation; the `write_result`/`write_saved`
   /// table arms stay unescaped at the seam and their callers own their interpolations.
   pub fn escape_terminal_controls(value: &str) -> String {
   ```

2. In `src/output/mod.rs`, add the re-export next to the existing module declarations
   (after `mod formatting;`, keeping the `pub use` grouped with the other public items):

   ```rust
   mod formatting;
   pub mod progress;
   pub(crate) mod resources;
   pub(crate) mod result_types;
   pub mod writers;

   /// Terminal-control escaping, re-exported for the command layer and the binary (ADR
   /// 0070). The `formatting` module is private; this is the public path.
   pub use formatting::escape_terminal_controls;
   ```

3. Run `make test-one T=escape_terminal_controls_escapes_cc_and_bidi_only`; expect it to pass
   unchanged. Commit.

## Task 2 — escape the batch-update stderr per-item error

Modifies `src/commands/bug/update/output.rs` and `src/commands/bug/update/output_tests.rs`.

**Interfaces.** Consumes `crate::output::escape_terminal_controls`. Changes
`write_batch_result`'s table arm: the per-item `f.error` is wrapped in
`escape_terminal_controls` before the `writeln!` to `w.err`. The JSON/NDJSON arm is unchanged
(it routes the `BatchResult` through `write_result` → `serde_json`).

**Verification.** Contract: a `Cc`/bidi character in a per-item error renders escaped in
table-mode stderr. `Mode: focused-test`. New
`write_batch_result_table_escapes_per_item_error` in `output_tests.rs`. Red: before the fix,
`io.err_str()` contains the raw `"\u{1b}"`, so the `assert_eq!` against the escaped spelling
fails. Green: `make test-one T=write_batch_result_table_escapes_per_item_error`.

1. Add the test first in `src/commands/bug/update/output_tests.rs` (it reuses the existing
   `BatchResult`, `BatchFailure`, `CapturedIo`, and `write_batch_result` imports already in the
   file):

   ```rust
   #[test]
   fn write_batch_result_table_escapes_per_item_error() {
       let batch = BatchResult::new(vec![1], vec![BatchFailure::new(2, "boom\u{1b}\u{202e}tail")]);
       let mut io = CapturedIo::new();

       write_batch_result(&batch, OutputFormat::Table, false, &mut io.writers());

       assert_eq!(io.out_str(), "Updated bugs: #1\n");
       assert_eq!(io.err_str(), "Failed to update bug #2: boom\\u{1b}\\u{202e}tail\n");
   }
   ```

   Run it; confirm the `err_str()` assertion fails (the raw `ESC` is present before the fix).

2. In `src/commands/bug/update/output.rs`, add the import
   `use crate::output::escape_terminal_controls;` and change the table arm's
   failure loop:

   ```rust
   for f in &batch.failed {
       let _ = writeln!(
           w.err,
           "Failed to update bug #{}: {}",
           f.id,
           escape_terminal_controls(&f.error)
       );
   }
   ```

3. Run `make test-one T=write_batch_result_table_escapes_per_item_error`; expect green. Run
   `make test-one T=write_batch_result` to confirm the existing update output tests still pass.
   Commit.

## Task 3 — escape the batch-create stderr per-item error

Modifies `src/commands/bug/create_json.rs` and `src/commands/bug/create_json_tests.rs`.

**Interfaces.** Consumes `crate::output::escape_terminal_controls`. Changes the
private `write_batch_create`'s table arm: the per-item `f.error` is wrapped in
`escape_terminal_controls` before the `writeln!` to `w.err`. The JSON/NDJSON arm is unchanged.

**Verification.** Contract: a `Cc`/bidi character in a batch-create per-item error renders
escaped in table-mode stderr. `Mode: focused-test`. New
`write_batch_create_table_escapes_per_item_error` in `create_json_tests.rs`. Red: before the
fix, `io.err_str()` contains the raw `"\u{1b}"`, so the `assert_eq!` against the escaped
spelling fails. Green:
`make test-one T=write_batch_create_table_escapes_per_item_error`.

1. Add the test first in `src/commands/bug/create_json_tests.rs` (the file does not yet import
   the result types or `CapturedIo`, so import them inside the test to avoid touching the
   shared import block):

   ```rust
   #[test]
   fn write_batch_create_table_escapes_per_item_error() {
       use crate::output::result_types::{BatchCreateResult, CreateFailure};
       use crate::test_helpers::CapturedIo;

       let result = BatchCreateResult::new(
           vec![1],
           vec![CreateFailure::create(0, "boom\u{1b}\u{202e}tail")],
       );
       let mut io = CapturedIo::new();

       super::write_batch_create(&result, OutputFormat::Table, &mut io.writers());

       assert_eq!(io.out_str(), "Created bugs: #1\n");
       assert_eq!(io.err_str(), "Failed to create bug (item 0): boom\\u{1b}\\u{202e}tail\n");
   }
   ```

   Run it; confirm the `err_str()` assertion fails (the raw `ESC` is present before the fix).

2. In `src/commands/bug/create_json.rs`, add the import
   `use crate::output::escape_terminal_controls;` and change the table arm's
   failure loop:

   ```rust
   for f in &result.failed {
       let _ = writeln!(
           w.err,
           "Failed to create bug (item {}): {}",
           f.index,
           escape_terminal_controls(&f.error)
       );
   }
   ```

3. Run `make test-one T=write_batch_create_table_escapes_per_item_error`; expect green. Commit.

## Task 4 — escape the `src/main.rs` `error: {…}` table renderings

Modifies `src/main.rs` and `src/main_tests.rs`.

**Interfaces.** Consumes `bzr::output::escape_terminal_controls` (the binary
reaches the library via `pub` items). Changes two `error: {…}` table renderings:
- The `resolve_format` failure arm (currently `writeln!(std::io::stderr(), "error: {e}")`).
- The `format_dispatch_error` Table arm (currently `format!("error: {err}")`).

The `format_dispatch_error` JSON and NDJSON arms are unchanged (they route the message through
`serde_json`).

**Verification.** Contract: a `Cc`/bidi character in a dispatch `BzrError` renders escaped in
the table arm, while the JSON/NDJSON arms are unchanged. `Mode: focused-test`. New
`format_dispatch_error_table_escapes_server_controls` in `main_tests.rs`. Red: before the fix,
the table arm contains the raw `"\u{1b}"`/`"\u{202e}"`, so the `assert!(table.contains("\\u{1b}"))`
fails. Green: `make test-one T=format_dispatch_error_table_escapes_server_controls`. The
`resolve_format` arm is `Mode: task-test-not-applicable`: it is inline in `main()` (not a
callable function) and renders `resolve_format`'s local validation errors, which carry no
server-controlled text; the change is a one-line wrap in the already-tested helper, and the
dispatch arm above carries the focused test.

1. Add the test first in `src/main_tests.rs` (it reuses the existing `BzrError`,
   `OutputFormat`, and `format_dispatch_error` references already in the file):

   ```rust
   #[test]
   fn format_dispatch_error_table_escapes_server_controls() {
       let err = BzrError::Api { code: 400, message: "hostile\u{1b}\u{202e}".into() };
       let table = format_dispatch_error(&err, OutputFormat::Table);
       assert!(table.starts_with("error: "), "{table}");
       assert!(table.contains("\\u{1b}"), "ESC must be escaped in table mode: {table}");
       assert!(table.contains("\\u{202e}"), "bidi must be escaped in table mode: {table}");
       assert!(!table.contains('\u{1b}'), "no raw ESC in table mode: {table}");
       assert!(!table.contains('\u{202e}'), "no raw bidi in table mode: {table}");

       // The JSON family is a published schema surface: serde_json escapes the ESC as
       // \u001b (code points below 0x20) but leaves the bidi override raw.
       let json = format_dispatch_error(&err, OutputFormat::Json);
       assert!(json.contains("\\u001b"), "serde must escape the ESC: {json}");
       assert!(
           json.contains('\u{202e}'),
           "serde leaves bidi raw (JSON family unchanged): {json}"
       );
   }
   ```

   Run it; confirm the table-arm assertions fail (raw `ESC`/bidi present before the fix) while
   the JSON assertions already pass.

2. In `src/main.rs`, add the import
   `use bzr::output::escape_terminal_controls;` and change the two table
   renderings:

   The `resolve_format` failure arm:

   ```rust
   Err(e) => {
       let _ = writeln!(
           std::io::stderr(),
           "error: {}",
           escape_terminal_controls(&e.to_string())
       );
       return exit_code(&e);
   }
   ```

   The `format_dispatch_error` Table arm:

   ```rust
   OutputFormat::Table => format!(
       "error: {}",
       escape_terminal_controls(&err.to_string())
   ),
   ```

3. Run `make test-one T=format_dispatch_error_table_escapes_server_controls`; expect green. Run
   `make test-one T=format_dispatch_error` to confirm the existing dispatch-error tests still
   pass. Commit.

## Task 5 — document the `write_result` composed-string contract

Modifies `src/output/result_types.rs` and `src/output/result_types_tests.rs`.

**Interfaces.** No signature change. Adds a doc comment to `write_result` recording that its
table arm prints `human_message` verbatim (a "print composed string" seam) and that callers
composing server-controlled data must escape their own interpolations (ADR 0070). The
`write_result` body is unchanged.

**Verification.** Contract: the `write_result` table arm still prints a `human_message`
containing `\n` verbatim (one line per `\n`), proving the seam did not start escaping. `Mode:
focused-test`. New `write_result_table_prints_newline_message_verbatim` in
`result_types_tests.rs`. Red: n/a — the seam was never escaping, so this passes before and
after; it is a regression guard that fails if a future edit makes the seam collapse newlines.
Green: `make test-one T=write_result_table_prints_newline_message_verbatim`. The doc comment
itself is `Mode: task-test-not-applicable` (prose; no executable consumer reads it).

1. Add the test first in `src/output/result_types_tests.rs`:

   ```rust
   #[test]
   fn write_result_table_prints_newline_message_verbatim() {
       use crate::test_helpers::CapturedIo;

       let mut io = CapturedIo::new();
       write_result(&[], "alpha\nbeta\ngamma", OutputFormat::Table, &mut io.writers());

       // The table arm prints the composed message verbatim — one line per `\n` — so
       // `src/commands/schema.rs`'s `names.join("\n")` listing is not collapsed.
       assert_eq!(io.out_str(), "alpha\nbeta\ngamma\n");
       assert_eq!(io.err_str(), "");
   }
   ```

   (Confirm `write_result` and `OutputFormat` are in scope in the sibling; add `use` lines if
   the sibling does not already import them.) Run it; confirm it passes (the seam is
   unchanged).

2. In `src/output/result_types.rs`, extend `write_result`'s doc comment:

   ```rust
   /// Print a mutation result: the typed payload under `--json`/`--output ndjson`, or the
   /// composed `human_message` verbatim in table mode. The table arm is a "print composed
   /// string" seam: it does not escape `human_message`, because its inputs are built by
   /// callers outside `src/output/` and `src/commands/schema.rs` relies on embedded newlines.
   /// A caller that composes server-controlled data into `human_message` must escape its own
   /// interpolations with `escape_terminal_controls` (ADR 0070).
   ```

3. Run `make test-one T=write_result_table_prints_newline_message_verbatim`; expect green.
   Commit.

## Task 6 — functional phase for batch-error escaping

Creates `tests/functional/phases/08i-batch-error-escaping.sh` and registers it in
`tests/functional/run-tests.sh`.

**Interfaces.** Consumes the `lib.sh` helpers (`make_bug`, `run_bzr`, `run_bzr_raw`,
`assert_stderr_contains`, `assert_stderr_not_contains`, `test_begin`/`test_pass`/`test_fail`/
`test_skip`, `BZR_STDOUT`, `BZR_EXIT`). No source interfaces.

**Verification.** Contract: against a real container, a per-item batch failure prints a
well-formed `Failed to update bug #…:` line and exits 11, and a hostile per-item error (when
the server round-trips it) reaches stderr escaped. `Mode: focused-test`.
`tests/functional/phases/08i-batch-error-escaping.sh`. Red: before the fix, the hostile case's
`assert_stderr_not_contains "$PATTERN_ESC"` fails (the raw `ESC` reaches stderr). Green:
`make functional-test` (runs the default Bugzilla version). The well-formed case also guards
the integration end-to-end.

1. Create `tests/functional/phases/08i-batch-error-escaping.sh`:

   ```bash
   # 08i-batch-error-escaping
   # Sourced by run-tests.sh in order; assumes lib.sh helpers and the
   # orchestrator preamble (constants, shared globals, cleanup trap).
   # Reads: none. Creates: its own bugs.
   # shellcheck shell=bash
   #
   # Terminal-control escaping in the batch command layer's per-item error line
   # (ADR 0070, issue #759). The per-item error is a BzrError display that can carry
   # server-supplied text, so a hostile one must reach stderr escaped. A wiremock fixture
   # can only prove "given this error string, we escape it"; only a real server proves the
   # error shape a Bugzilla actually returns still reaches the stderr line as a control
   # character. The hostile case round-trips the payload through --json first and skips
   # when the server does not echo it in the error, so a container that normalises the
   # input reports a named skip rather than a vacuous pass.

   echo "── Phase 8i: Batch-error terminal escaping ─────────────────"

   # Bash 3.2 (macOS system bash) has no $'\uXXXX'; PATTERN_ESC is the BRE-safe form of
   # PAYLOAD_ESC for the `grep`-based assertion helpers (they run without -F).
   _BE_ARGS=(--product FuncTestProd --component Backend --op-sys Linux --platform PC
       --description "batch error escaping probe")
   PAYLOAD_ESC=$'\x1b[2J'
   PATTERN_ESC=$'\x1b''\[2J'

   BE_A=$(make_bug "${_BE_ARGS[@]}" --summary "batch error probe A")
   BE_B=$(make_bug "${_BE_ARGS[@]}" --summary "batch error probe B")

   test_begin "batch-error-well-formed" "a per-item batch failure prints a well-formed stderr line"
   if [[ -z "$BE_A" || -z "$BE_B" ]]; then
       test_fail "could not create the probe bugs"
   else
       # An invalid resolution fails both items; the batch still reports each failure on
       # stderr and exits with the batch-partial-failure code (11).
       run_bzr_raw --output table bug update "$BE_A" "$BE_B" \
           --status RESOLVED --resolution "not-a-real-resolution"
       if [[ $BZR_EXIT -eq 11 ]] &&
           assert_stderr_contains "Failed to update bug #$BE_A:" &&
           assert_stderr_contains "Failed to update bug #$BE_B:"; then
           test_pass
       fi
   fi

   test_begin "batch-error-hostile-escaped" "a hostile per-item error reaches stderr escaped"
   if [[ -z "$BE_A" || -z "$BE_B" ]]; then
       test_fail "could not create the probe bugs"
   else
       # Round-trip probe: does the server echo the hostile resolution in the per-item
       # error? The JSON batch result carries the raw server text; serde escapes code
       # points below 0x20, so a stored ESC appears there as \u001b.
       run_bzr bug update "$BE_A" "$BE_B" \
           --status RESOLVED --resolution "$PAYLOAD_ESC"
       if ! grep -q 'u001b' "$BZR_STDOUT" 2>/dev/null; then
           test_skip "the server did not echo the ESC in the per-item error; the unit tests carry the escape contract"
       else
           run_bzr_raw --output table bug update "$BE_A" "$BE_B" \
               --status RESOLVED --resolution "$PAYLOAD_ESC"
           if assert_stderr_not_contains "$PATTERN_ESC" && assert_stderr_contains 'u{1b}'; then
               test_pass
           fi
       fi
   fi

   unset BE_A BE_B PAYLOAD_ESC PATTERN_ESC _BE_ARGS
   ```

2. Register the phase in `tests/functional/run-tests.sh` in source order, immediately after
   `08h-terminal-escaping.sh` (match the existing `source`/`bash` line shape used for the 08x
   phases). Confirm the id `08i-batch-error-escaping` is unique.

3. Run `make functional-test`; expect both cases to pass (or the hostile case to report its
   named skip when the container normalises the ESC). Run `make check-shell` and
   `make check-functional-test-ids`. Commit.

## Task 7 — docs: CLI reference and the ADR 0070 index row

Modifies `docs/bzr-cli.md` and `docs/adr/README.md`.

**Interfaces.** No code interfaces. Adds a note to `docs/bzr-cli.md` that table-mode stderr
error messages (the batch per-item failure lines and the `error: {…}` dispatch line) escape
terminal control characters, and appends the ADR 0070 row to `docs/adr/README.md`.

**Verification.** `Mode: task-test-not-applicable`. Both edits are prose whose only consumer is
a human reader; the repository has no doc-content gate that reads either file's body, so no
executable or structural observation over them could fail meaningfully.

1. In `docs/bzr-cli.md`, add a short note beside the existing terminal-escaping documentation
   (the section ADR 0065 / issue #743 added) stating that the batch command layer's per-item
   failure lines and the `error: {…}` dispatch line now escape `Cc` and the Trojan-Source bidi
   set the same way the writers do.

2. In `docs/adr/README.md`, append one row after the 0069 row, matching the surrounding rows'
   length and tone, with the `Status` cell `Accepted`:

   ```
   | [0070](0070-escape-terminal-controls-at-composition-sites.md) | Escape terminal controls at message-composition sites outside the writers | Accepted |
   ```

3. Run `make lint`; expect green. Commit.

## Rollback

Every task is an independent, revertible commit: the visibility widening, the three sink
escapes, the seam doc comment, the functional phase, and the docs. There is no migration, no
persisted state, and no published-schema change to unwind. Reverting any task restores the
prior (unescaped-at-that-site) behaviour without affecting the others.
