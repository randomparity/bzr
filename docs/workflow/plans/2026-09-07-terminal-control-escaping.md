# Plan: terminal-control escaping across every output writer

**Goal.** Make every `src/output/` writer escape Unicode `Cc` plus the Trojan-Source
bidi set before server-controlled text reaches the terminal.

**Architecture.** One predicate in `src/output/formatting.rs`
(`escape_terminal_controls`) applied at three seams — `write_table_records`, the
`write_field` family, and a new `write_status_field` — plus an explicit call at each
writer that composes its own `writeln!`. Cells are truncated first, escaped last.
`write_table` is folded into `write_table_records` so no escape-free table entry
point survives.

Design: `docs/workflow/specs/2026-09-07-terminal-control-escaping-design.md`.
Decision: `docs/adr/0065-escape-terminal-controls-in-every-writer.md`.
Size band M: ~150 source lines, ~230 lines of unit cases across 14 `*_tests.rs`
siblings, ~70 lines of docs plus one functional phase. Tests dominate, as the frozen
assessment's provenance already noted; this is not a re-scope.

## Global constraints

- Unit tests in sibling `<name>_tests.rs` (`#[cfg(test)] #[path = …] mod tests;`);
  inline `mod tests` in `src/` is forbidden (`make check-test-layout`).
- Never call `colored::control::set_override` in a test — process-global, flakes
  parallel tests. Inspect `ColoredString.fgcolor` instead.
- Output via `Writers` / the `output` helpers, never `println!`. Clippy pedantic,
  `-D warnings`; `unwrap_used` denied.
- Guardrails `make lint`, `make test`, `make functional-test`; iterate with
  `make test-one T=<substr>` / `make test-fast`. Never bare `cargo test`.
- Phase scripts: 4-space indent, `# shellcheck shell=bash` header, gated by
  `make check-shell`; ids `^[a-z0-9]+(-[a-z0-9]+)*$`, globally unique
  (`tools/check-functional-test-ids.sh`).
- Do not edit `docs/adr/README.md` (orchestrator owns the row), `src/client/**`,
  `src/http.rs`, `src/xmlrpc/**` (issue #740), or `src/config/**` (issue #738).

## File map

| File | Answerable for |
|---|---|
| `src/output/formatting.rs` (+ sibling) | predicate, three seams, corrected doc comment |
| `src/output/resources/bug.rs` | four `Builder` sites, Status row, header, history, unavailable block |
| `src/output/resources/comment.rs` | header line, body lines, renamed tag call |
| `src/output/resources/field.rs` | drop the now-doubled explicit escape |
| `resources/{attachment,product,classification,component,group,user,server,config,template,query}.rs` | their own `writeln!` interpolations |
| `src/output/resources/*_tests.rs` (14) | one escaping case per writer |
| `tests/functional/phases/08h-terminal-escaping.sh`, `run-tests.sh` | end-to-end proof |
| `docs/bzr-cli.md` | the rendering change |

## Task 1 — widen and rename the shared predicate

Modifies `src/output/formatting.rs`, `formatting_tests.rs`, and the two existing call
sites (`resources/field.rs:71`, `resources/comment.rs:40`).

**Interfaces.** Produces `pub(super) fn escape_terminal_controls(&str) -> String`;
removes `escape_table_control`. Every later task consumes it.

**Verification.** Contract: the escaped character set. `Mode: focused-test`.
`escape_terminal_controls_escapes_cc_and_bidi_only` in `formatting_tests.rs`. Red: the
`\u{202e}` assertion fails because `U+202E` passes through. Green:
`make test-one T=escape_terminal_controls_escapes_cc_and_bidi_only`.

1. Add the test first, over one string mixing `\u{1b}`, `\t`, each bidi code point,
   `\u{200c}`, `\u{200b}`, `\u{feff}`, `é`, and a literal backslash. It pins two
   properties: the permitted format characters and ordinary text survive unchanged,
   and `U+061C` renders `\u{61c}` — `char::escape_default` does not zero-pad. Run it;
   confirm the raw-`U+202E` assertion fails.
2. Add the constant and rename the function, changing exactly one line of its body —
   `if character.is_control()` becomes
   `if character.is_control() || BIDI_CONTROLS.contains(&character)`. The loop, the
   `escape_default()` call, the `else` arm, and the `with_capacity` allocation are
   unchanged.

   ```rust
   const BIDI_CONTROLS: [char; 12] = [
       '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}',
       '\u{2067}', '\u{2068}', '\u{2069}', '\u{200e}', '\u{200f}', '\u{61c}',
   ];
   ```

3. Rewrite the doc comment to state the two categories, why the rest of `Cf` is
   absent, the three seams, and that the JSON family is not covered because
   `serde_json` escapes only `"`, `\`, and code points below `0x20` — the claim the
   old comment got wrong. Cite ADR 0065.
4. Rename the two call sites and their `use` lists so the crate compiles.
5. Focused test green; `make lint`; commit
   `refactor(output): widen and rename the terminal-escaping predicate`.

**Acceptance.** `rg escape_table_control src/` is empty; no doc comment claims serde
escapes control characters generally.

## Task 2 — escape at the table seam and remove the bypass

Modifies `src/output/formatting.rs` (+ sibling), `resources/bug.rs`,
`resources/field.rs`.

**Interfaces.** `write_table_records` keeps its signature; `write_table` is removed.

**Verification.** Contract: every cell and header is escaped. `Mode: focused-test`.
`write_table_records_escapes_cells_and_headers` in `formatting_tests.rs`. Red: a raw
`\u{1b}` remains in the rendered table. Green:
`make test-one T=write_table_records_escapes`.

1. Add the test — a header and a cell each carrying `\u{1b}[2J` and `\u{202e}`; assert
   the rendered text contains neither raw character and contains both escaped forms.
   Run it; confirm red.
2. Fold `write_table` into `write_table_records`, mapping
   `escape_terminal_controls` over headers and over each row's cells as they are
   pushed, then inlining `write_table`'s three statements (build, optional
   `Width::wrap(width.max(minimum_width)).priority(PriorityMax::right())`, `writeln!`)
   verbatim. Delete `write_table` and any `use` only it needed. Escaping runs after
   each `to_record` closure, so `truncate` still counts original characters.
3. In `resources/field.rs`, drop the now-doubled explicit escape on `row.name` — the
   seam does it, and keeping it would double-escape a backslash. Update the comment
   above it to say the seam escapes both cells.
4. In `resources/bug.rs`, migrate the four `Builder::default()` sites onto
   `write_table_records`, dropping the `Builder` and `write_table` imports. Headers and
   rows are exactly the arrays each site already pushes, collected as `Vec<String>`.
   `write_bugs` needs a `Vec<String>` of headers plus a `Vec<&str>` view, because
   `SelectedBugField::header` returns `String` (`src/types/bug/fields.rs:221`).
5. Focused test green; `make test-fast`; fix any moved table assertion. `make lint`;
   commit `fix(output): escape every table cell at the shared table seam`.

**Acceptance.** `rg "Builder::default" src/output/` matches only `formatting.rs`;
`write_table` does not exist.

## Task 3 — escape at the detail seam, and keep the status colour

Modifies `src/output/formatting.rs` (+ sibling), `resources/bug.rs`.

**Interfaces.** Produces
`pub(super) fn write_status_field<W: Write + ?Sized>(out: &mut W, label: &str, status: &str)`,
consumed only by `resources/bug.rs`.

**Verification.**

- Labels and values escaped in all three helpers. `Mode: focused-test`.
  `write_field_family_escapes_labels_and_values` in `formatting_tests.rs`. Red: raw
  ESC in the emitted rows. Green: `make test-one T=write_field_family_escapes`.
- The Status row escapes and still colours. `Mode: focused-test`.
  `bug_detail_status_escapes_but_keeps_colour` in `resources/bug_tests.rs`, asserting
  on `colorize_status(<escaped>).fgcolor` rather than emitted ANSI bytes. Red: the
  colour is escaped, or the ESC survives. Green:
  `make test-one T=bug_detail_status_escapes`.

1. Add `write_field_family_escapes_labels_and_values`, exercising `write_field`,
   `write_optional_field`, and `write_list_field` with a control-bearing label and
   values. Run it; confirm red.
2. Escape inside `write_field` — both the label and the value — and collapse the other
   two onto it. Each keeps its own signature and its own guard (`unwrap_or("-")`, the
   `is_empty` early return) but delegates the `writeln!`, so exactly one place escapes.
3. Add `write_status_field` beside them: escape the label, escape the status, then
   `colorize_status` the escaped text, so the only ANSI emitted is bzr's own.
   Doc-comment it as the one value that arrives needing colour, citing ADR 0065.
4. In `resources/bug.rs`, make `DetailValue::Status` yield the raw status and route
   that one field through the new seam inside `write_bug_detail_table`, branching on
   `detail.field == BugField::Status` (`BugField` is `Copy + Eq`,
   `src/types/bug/fields.rs:29`).
5. Both focused tests green; `make lint`; commit
   `fix(output): escape labels and values at the detail-row seam`.

**Acceptance.** Every `write_field`-family caller inherits escaping with no call-site
change; `bug view` still colours its Status row.

## Task 4 — escape the writers that compose their own lines

Modifies, under `src/output/resources/`: `bug.rs`, `comment.rs`, `attachment.rs`,
`product.rs`, `classification.rs`, `component.rs`, `group.rs`, `user.rs`, `server.rs`,
`config.rs`, `template.rs`, `query.rs`.

**Verification.** Contract: each writer's bespoke lines escape server text.
`Mode: focused-test`. Covered by Task 5's per-writer cases, written first for each file
touched here.

Edit shape at every site: wrap the interpolation in `escape_terminal_controls(…)`,
keeping `.bold()` / `.cyan()` **outside** it, e.g.
`escape_terminal_controls(bug.summary.as_deref().unwrap_or("unknown")).bold()`.

Walk each file's `writeln!` sites and wrap every interpolated `types::` value that is
not a number or a `&'static str` literal. Specifically: `bug.rs`'s detail header
summary, `write_history_table`, and `write_unavailable_block`; `comment.rs`'s header
and each body line; `attachment.rs`'s header summary and the batch table's paths and
errors; `product.rs`'s named lists and detail block; `classification.rs`'s header and
per-product rows; `component.rs`, `group.rs`, `user.rs` headers and member rows;
`server.rs`'s version, extensions, capabilities, transitions, and custom fields;
`config.rs`'s config path, default, and `[{name}]` header; and the composed lines in
`template.rs` / `query.rs` (local config values rather than server ones — one call
each, same seam, no second standard). Custom detail fields already route through
`write_field`; confirm only.

Then `make test-fast`, `make lint`; commit
`fix(output): escape server text in the bespoke writer lines`.

**Acceptance.** `rg -n 'writeln!' src/output/resources/` shows no remaining
interpolation of a `types::` field that is not a number, a `&'static str` literal, or
wrapped in `escape_terminal_controls`.

## Task 5 — one escaping case per writer

Modifies the 14 `*_tests.rs` siblings under `src/output/resources/`, plus
`formatting_tests.rs`.

**Verification.** Contract: no writer emits a raw `U+001B` or `U+202E` from server data
in table mode. `Mode: focused-test`. One case per sibling named
`<writer>_table_escapes_terminal_controls`. Red for any writer whose Task 2/4 edit is
missing: the raw character appears. Green: `make test-one T=escapes_terminal_controls`.

1. For each of `bug`, `comment`, `attachment`, `product`, `classification`,
   `component`, `group`, `user`, `server`, `field`, `config`, `template`, `query`,
   `skills`, add one test reusing that file's existing capture helper (each has one;
   read the file and reuse rather than adding a second). Build the fixture that file
   already builds with every server-controlled string field set to
   `format!("ev\u{1b}[2Jil\u{202e}{n}")`, render in `OutputFormat::Table`, and assert
   the text contains neither raw character and does contain both escaped forms, each
   with the captured text in the message. `bug_tests.rs` covers its six entry points —
   `write_bugs`, `write_bug_links`, `write_bug_adjacency`, `write_bug_detail`,
   `write_history_table`, `write_multi_bug_view` — in one case, since each renders
   separately.
2. Prove each case bites: revert one writer's Task 2/4 edit, run
   `make test-one T=escapes_terminal_controls`, observe that case fail, restore. Name
   the fault-checked writers in the commit message.
3. Correct the doc comment above `write_field_names_table_escapes_control_characters`
   in `field_tests.rs`: it says "JSON is unaffected — serde escapes it there", true
   only for `Cc`. Replace with a sentence naming what `serde_json` escapes and
   pointing at ADR 0065.
4. Add `json_family_output_is_not_escaped_for_bidi` to `formatting_tests.rs`: render a
   `U+202E`-bearing value through `write_json` and `write_ndjson`, assert the raw
   character is still present, so the escape cannot quietly migrate into the JSON arm.
5. `make test`, `make lint`; commit `test(output): cover terminal escaping in every writer`.

**Acceptance.** Every writer under `src/output/resources/` has one escaping case, each
observed red against a deliberately reverted edit.

## Task 6 — document the change and prove it against a container

Creates `tests/functional/phases/08h-terminal-escaping.sh`; modifies
`tests/functional/run-tests.sh`, `docs/bzr-cli.md`.

**Interfaces.** Consumes `test_begin`, `test_pass`, `test_skip`, `run_bzr`,
`run_bzr_raw`, `assert_success`, `assert_stdout_contains`,
`assert_stdout_not_contains`, and `$BZR_STDOUT`, all from `tests/functional/lib.sh`.
Product and component are the container fixtures `FuncTestProd` and `Backend`, spelled
as literals exactly as `tests/functional/phases/08-bugs.sh:12` does — the harness
defines no `$PRODUCT`/`$COMPONENT` global.

**Verification.**

- A real Bugzilla round-trip puts no raw control or bidi character on the terminal.
  `Mode: focused-test`. The phase's four cases. Red against a build without Tasks 2–4:
  `assert_stdout_not_contains` fails on the raw character. Green:
  `make functional-test`.
- `docs/bzr-cli.md` records the rendering change. `Mode: task-test-not-applicable`.
  Prose for a human reader; `tools/` contains no doc-content check that reads its body,
  so no executable or structural observation over it could fail meaningfully.

1. Write the phase with ids `escape-summary-bidi`, `escape-summary-esc`,
   `escape-comment-body`, `escape-json-passthrough`. Each case creates a bug (or
   comment) carrying the payload, reads it back through `--json` to confirm the server
   stored it, and only then asserts on table output: raw payload absent, escaped form
   present. The round-trip guard is deliberate — if the container's Bugzilla normalises
   the payload away, `test_skip` with a named reason rather than passing vacuously or
   failing on server behaviour bzr does not control. The fourth case asserts
   `bzr bug view --json` still carries the raw bidi character, pinning the exclusion
   this change deliberately does not close. Note that every `assert_*` helper calls the
   non-idempotent `test_fail` and returns non-zero, so chain them with `&&` inside one
   `if` rather than pairing an assertion with an else-`test_fail`.
2. Register `08h-terminal-escaping` in `run-tests.sh`'s phase list, immediately after
   `08g-bug-arbitrary-fields`.
3. `make check-shell`, `bash -n` the new phase, and `tools/check-functional-test-ids.sh .`
   — all clean, ids unique.
4. In `docs/bzr-cli.md`, add a paragraph to the output-format section: table output
   escapes C0/C1 controls and Trojan-Source bidi controls in server-supplied values as
   `\u{…}`; `--json`/`--output ndjson` are unchanged; a tab inside a comment body now renders
   as `\t`.
5. `make lint`, `make test`, then `make functional-test`; report the Bugzilla version
   that ran.
6. Commit `test(functional): prove terminal escaping against a live server` and
   `docs(output): record the terminal-escaping rendering change`.

**Acceptance.** `make functional-test` green with the new phase running (or skipping
with a named server-normalisation reason); `docs/bzr-cli.md` describes the rendering.

## Rollback

Each task is a separate commit touching only `src/output/**`, `docs/**`, and
`tests/functional/**`. `git revert` over the range restores the previous rendering: no
migration, no persisted state, no published-schema change to unwind.
