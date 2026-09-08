# Plan: terminal-control escaping across every output writer

**Goal.** Make every `src/output/` writer escape Unicode `Cc` plus the Trojan-Source
bidi set before server-controlled text reaches the terminal.

**Architecture.** One predicate in `src/output/formatting.rs`
(`escape_terminal_controls`) applied at three seams — `write_table_records`, the
`write_field` family, and a new `write_status_field` — plus an explicit call at each
writer that composes its own `writeln!`. Cells are truncated first, escaped last.
`write_table` is folded into `write_table_records` so no escape-free table entry
point survives.

**Tech stack.** Rust 2021; `tabled` tables, `colored` status colour, `tokio` async
tests, `wiremock` HTTP mocks, bash functional phases.
Design: `docs/workflow/specs/2026-09-07-terminal-control-escaping-design.md`.
Decision: `docs/adr/0065-escape-terminal-controls-in-every-writer.md`.

Expected implementation size: 380–520 changed lines (M) — from the file map:
~150 source lines across `formatting.rs` and 12 writers, ~230 lines of unit cases
across 14 `*_tests.rs` siblings, ~70 lines of docs plus one functional phase. The
band stays M because the frozen assessment's provenance already named "per-category
unit cases across 14 writer siblings" as its reason; tests dominate the estimate.
This is not a re-scope.

## Global Constraints

- Unit tests live in sibling `<name>_tests.rs` files linked with
  `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`; inline `mod tests` in `src/`
  is forbidden (`make check-test-layout`). Siblings start with
  `#![expect(clippy::unwrap_used)]` where needed.
- Never call `colored::control::set_override` in a test — process-global, flakes
  parallel tests. Inspect `ColoredString.fgcolor` instead.
- Output goes through `Writers` / the `output` helpers, never `println!`. Clippy
  pedantic, `-D warnings`; `unwrap_used` denied, `expect_used` and `allow_attributes`
  warned.
- Guardrails `make lint`, `make test`, `make functional-test`; iterate with
  `make test-one T=<substr>` / `make test-fast`. Never bare `cargo test`.
- Phase scripts: 4-space indent, `# shellcheck shell=bash` header, gated by
  `make check-shell`; ids match `^[a-z0-9]+(-[a-z0-9]+)*$` and are globally unique
  (`tools/check-functional-test-ids.sh`).
- Do not edit `docs/adr/README.md` (orchestrator owns the row), `src/client/**`,
  `src/http.rs`, or `src/xmlrpc/**` (issue #740).

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

**Interfaces.** Produces
`pub(super) fn escape_terminal_controls(value: &str) -> String`; removes
`escape_table_control`. Every later task consumes it.

**Verification.** Contract: the escaped character set. `Mode: focused-test`.
`escape_terminal_controls_escapes_cc_and_bidi_only` in `formatting_tests.rs`. Red:
the `\u{202e}` assertion fails because `U+202E` passes through. Green:
`make test-one T=escape_terminal_controls_escapes_cc_and_bidi_only`.

1. Add the test:

   ```rust
   #[test]
   fn escape_terminal_controls_escapes_cc_and_bidi_only() {
       let escaped = escape_terminal_controls(
           "a\u{1b}b\tc\u{202e}d\u{2066}e\u{200f}f\u{61c}g\u{200c}h\u{200b}i\u{feff}jé\\",
       );
       assert_eq!(
           escaped,
           "a\\u{1b}b\\tc\\u{202e}d\\u{2066}e\\u{200f}f\\u{61c}g\u{200c}h\u{200b}i\u{feff}jé\\",
           "must cover Cc and the Trojan-Source bidi set and nothing else"
       );
   }
   ```

   It pins two properties: `U+200C`, `U+200B`, `U+FEFF`, `é`, and a backslash survive
   unchanged; `U+061C` renders `\u{61c}` — `char::escape_default` does not zero-pad.
2. `make test-one T=escape_terminal_controls_escapes_cc_and_bidi_only`; confirm the
   raw-`U+202E` assertion fails.
3. Add the constant, and rename `escape_table_control` to `escape_terminal_controls`
   changing exactly one line of its body — its `if character.is_control()` condition
   becomes `if character.is_control() || BIDI_CONTROLS.contains(&character)`. The
   loop, the `escape_default()` call, the `else { escaped.push(character) }` arm, and
   the `String::with_capacity(value.len())` allocation are unchanged.

   ```rust
   const BIDI_CONTROLS: [char; 12] = [
       '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}',
       '\u{2067}', '\u{2068}', '\u{2069}', '\u{200e}', '\u{200f}', '\u{61c}',
   ];
   ```

   Rewrite the doc comment to state: the two categories; why the rest of `Cf` is
   absent; the three seams it is applied at; and that the JSON family is not covered
   because `serde_json` escapes only `"`, `\`, and code points below `0x20` — the
   claim the old comment got wrong. Cite ADR 0065.
4. Rename the two call sites and their `use` lists so the crate compiles.
5. Focused test green; `make lint`; commit
   `refactor(output): widen and rename the terminal-escaping predicate`.

**Acceptance.** `rg escape_table_control src/` is empty; no doc comment claims serde escapes control characters generally.

## Task 2 — escape at the table seam and remove the bypass

Modifies `src/output/formatting.rs` (+ sibling), `resources/bug.rs`,
`resources/field.rs`.

**Interfaces.** `write_table_records(headers: &[&str], rows: impl IntoIterator<Item = Vec<String>>, width: Option<usize>, out: &mut W)`
keeps its signature; `write_table` is removed.

**Verification.** Contract: every cell and header is escaped. `Mode: focused-test`.
`write_table_records_escapes_cells_and_headers` in `formatting_tests.rs`. Red: a raw
`\u{1b}` remains in the rendered table. Green:
`make test-one T=write_table_records_escapes`.

1. Add the test: call
   `write_table_records(&["N\u{202e}AME"], vec![vec!["ev\u{1b}[2Jil\u{202e}".to_string()]], None, &mut buf)`,
   assert the text contains neither raw character and contains both `\\u{1b}` and
   `\\u{202e}`. Run it; confirm red.
2. Fold `write_table` into `write_table_records`, escaping as the records are pushed.
   Keep the existing signature and body, making three changes: the header push
   becomes
   `builder.push_record(headers.iter().map(|header| escape_terminal_controls(header)));`,
   the row push becomes
   `builder.push_record(row.iter().map(|cell| escape_terminal_controls(cell)));`, and
   `write_table(builder.build(), width, out)` is replaced by `write_table`'s own three
   statements inlined verbatim — `let mut table = builder.build();`, the
   `if let Some(width) = width { … Width::wrap(width.max(minimum_width)).priority(PriorityMax::right()) … }`
   block, and `let _ = writeln!(out, "{table}");`.

   Delete `write_table` and any `use` only it needed. Escaping runs after each
   `to_record` closure, so `truncate` still counts original characters.
3. In `resources/field.rs`, drop the explicit `escape_terminal_controls(&row.name)` —
   the seam does it, and keeping it would double-escape a backslash. Use
   `vec![row.name.clone(), row.source.as_str().to_string()]`, and update the comment
   above it to say the seam escapes both cells.
4. In `resources/bug.rs`, migrate the four `Builder::default()` sites onto
   `write_table_records`, dropping the `Builder` and `write_table` imports. Headers
   and rows are exactly the arrays each site already pushes, collected as
   `Vec<String>`: `write_bugs` (~170) builds
   `let headers: Vec<String> = columns.iter().map(|f| (*f).header()).collect();` then
   `let header_refs: Vec<&str> = headers.iter().map(String::as_str).collect();`
   (`SelectedBugField::header` returns `String`, `src/types/bug/fields.rs:221`);
   `write_bug_links` (~453) uses `["ID","RELATION","DIR","DEPTH","STATUS","SUMMARY"]`;
   adjacency requests (~502) `["REQUESTED","RESULT"]` with row
   `vec![requested.clone(), outcome]`; adjacency bugs (~516) its existing 11 headers
   and 11-element row.
5. Focused test green; `make test-fast`; fix any moved table assertion. `make lint`;
   commit `fix(output): escape every table cell at the shared table seam`.

**Acceptance.** `rg "Builder::default" src/output/` matches only `formatting.rs`; `write_table` does not exist.

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

1. Add `write_field_family_escapes_labels_and_values`, calling `write_field`,
   `write_optional_field`, and `write_list_field` with label `"la\u{1b}bel"` and
   values carrying `\u{1b}` and `\u{202e}`; assert neither raw character appears and
   both escaped forms do. Run it; confirm red.
2. Escape inside `write_field`, and collapse the other two onto it (each keeps its
   own signature and its own guard — `unwrap_or("-")` and the `is_empty` early
   return — but delegates the `writeln!`), so exactly one place escapes:

   ```rust
   pub(super) fn write_field<W: Write + ?Sized>(out: &mut W, label: &str, value: &str) {
       let label = escape_terminal_controls(label);
       let _ = writeln!(out, "  {label:<12}  {}", escape_terminal_controls(value));
   }
   ```

   `write_optional_field` becomes `write_field(out, label, value.unwrap_or("-"))`;
   `write_list_field` becomes `write_field(out, label, &items.join(", "))` inside its
   existing `if !items.is_empty()`.
3. Add the coloured seam beside them, doc-commented as the one value that arrives
   pre-coloured (escape first, colour second, so the only ANSI is bzr's own; ADR 0065):

   ```rust
   pub(super) fn write_status_field<W: Write + ?Sized>(out: &mut W, label: &str, status: &str) {
       let label = escape_terminal_controls(label);
       let _ = writeln!(
           out,
           "  {label:<12}  {}",
           colorize_status(&escape_terminal_controls(status))
       );
   }
   ```
4. In `resources/bug.rs`, make `DetailValue::Status` yield the raw status
   (`bug.status.as_deref().unwrap_or("-").to_string()`) and route that one field
   through the new seam inside `write_bug_detail_table`:

   ```rust
   if let Some(row) = render_builtin_detail_field(*detail, bug) {
       if detail.field == BugField::Status {
           write_status_field(out, row.label, &row.value);
       } else {
           write_field(out, row.label, &row.value);
       }
   }
   ```

   `BugField` derives `Clone, Copy, Debug, Eq, PartialEq`
   (`src/types/bug/fields.rs:29`), so `==` is available.
5. Both focused tests green; `make lint`; commit
   `fix(output): escape labels and values at the detail-row seam`.

**Acceptance.** Every `write_field`-family caller inherits escaping with no call-site change; `bug view` still colours its Status row.

## Task 4 — escape the writers that compose their own lines

Modifies, under `src/output/resources/`: `bug.rs`, `comment.rs`, `attachment.rs`,
`product.rs`, `classification.rs`, `component.rs`, `group.rs`, `user.rs`, `server.rs`,
`config.rs`, `template.rs`, `query.rs`.

**Interfaces.** Consumes `escape_terminal_controls`; each file adds it to its
`use crate::output::formatting::{…}` list.

**Verification.** Contract: each writer's bespoke lines escape server text.
`Mode: focused-test`. Covered by Task 5's per-writer cases, written first for each
file touched here.

Edit shape at every site: wrap the interpolation in `escape_terminal_controls(…)`,
keeping `.bold()` / `.cyan()` **outside** it, e.g.
`escape_terminal_controls(bug.summary.as_deref().unwrap_or("unknown")).bold()`.

1. `bug.rs` — the detail header's `bug.summary`; `write_history_table`'s `entry.who`,
   `entry.when`, `change.field_name`, `removed`, `added`; `write_unavailable_block`'s
   `id` and `error`. Custom detail fields already route through `write_field`; confirm
   only.
2. `comment.rs` — the header's `c.creator` and `c.creation_time`, and each body line
   in the `for line in …lines()` loop.
3. `attachment.rs` — `write_attachment_header`'s `a.summary`; the batch table's
   `file.path` (both occurrences), `att.path`, `bug.error`, `att.error`.
4. `product.rs` — `format_named_list`'s `name.as_ref()`; `format_product_detail`'s
   product name and description, each component name, each `assignee`.
5. `classification.rs` — the header's name and description, each product's name and
   truncated description.
6. `component.rs` — the header's `c.name`. `group.rs` — the header's `group.name` and
   each member's name and `real_name`. `user.rs` — `write_whoami`'s
   `whoami.identity.name`.
7. `server.rs` — `info.version`; each extension `name` and `ver`; `caps.version`; the
   joined `api_modes` and `auth_modes`; each `transition.from` and joined
   `can_change_to`; each custom field's `name`, `field_type`, joined `values`.
8. `config.rs` — `v.config_file`, `def`, and `write_server`'s `name` in `[{name}]`.
   `template.rs` / `query.rs` — escape the composed line once at each `writeln!` in
   `write_template_list` / `write_query_list`. These are local config values rather
   than server ones; one call each, same seam, no second standard.
9. `make test-fast`, `make lint`; commit
   `fix(output): escape server text in the bespoke writer lines`.

**Acceptance.** `rg -n 'writeln!' src/output/resources/` shows no remaining interpolation of a `types::` field that is not a number, a `&'static str` literal, or wrapped in `escape_terminal_controls`.

## Task 5 — one escaping case per writer

Modifies the 14 `*_tests.rs` siblings under `src/output/resources/`, plus
`formatting_tests.rs`.

**Interfaces.** Consumes each writer's public entry point; adds no production symbol.

**Verification.** Contract: no writer emits a raw `U+001B` or `U+202E` from server
data in table mode. `Mode: focused-test`. One case per sibling named
`<writer>_table_escapes_terminal_controls`. Red for any writer whose Task 2/4 edit is
missing: the raw character appears. Green:
`make test-one T=escapes_terminal_controls`.

1. For each of `bug`, `comment`, `attachment`, `product`, `classification`,
   `component`, `group`, `user`, `server`, `field`, `config`, `template`, `query`,
   `skills`, add one test reusing that file's existing capture helper (each has one —
   `capture` in `comment_tests.rs`, `capture_names` in `field_tests.rs`; read the file
   and reuse rather than adding a second). Build the fixture that file already builds,
   with every server-controlled string field set to
   `format!("ev\u{1b}[2Jil\u{202e}{n}")`, render it in `OutputFormat::Table`, and
   assert four things: the text contains neither `'\u{1b}'` nor `'\u{202e}'`, and does
   contain `"\\u{1b}"` and `"\\u{202e}"`, each with the captured text in the message.
   `bug_tests.rs` covers six entry points in its case — `write_bugs`,
   `write_bug_links`, `write_bug_adjacency`, `write_bug_detail`,
   `write_history_table`, `write_multi_bug_view` — since each renders separately.
2. Prove each case bites: revert one writer's Task 2/4 edit, run
   `make test-one T=escapes_terminal_controls`, observe that case fail, restore. Name
   the fault-checked writers in the commit message.
3. Correct the doc comment above
   `write_field_names_table_escapes_control_characters` in `field_tests.rs`: it says
   "JSON is unaffected — serde escapes it there", true only for `Cc`. Replace with a
   sentence naming what `serde_json` escapes and pointing at ADR 0065.
4. Add `json_family_output_is_not_escaped_for_bidi` to `formatting_tests.rs`: render a
   value carrying `U+202E` through `write_json` and `write_ndjson`, assert the raw
   character is still present, so the escape cannot quietly migrate into the JSON arm.
5. `make test`, `make lint`; commit
   `test(output): cover terminal escaping in every writer`.

**Acceptance.** Every writer under `src/output/resources/` has one escaping case, each observed red against a deliberately reverted edit.

## Task 6 — document the change and prove it against a container

Creates `tests/functional/phases/08h-terminal-escaping.sh`; modifies
`tests/functional/run-tests.sh`, `docs/bzr-cli.md`.

**Interfaces.** Consumes `test_begin`, `test_pass`, `test_skip`, `run_bzr`,
`run_bzr_raw`, `assert_success`, `assert_stdout_contains`,
`assert_stdout_not_contains`, and `$BZR_STDOUT`, all from `tests/functional/lib.sh`.
Product and component are the container fixtures `FuncTestProd` and `Backend`,
spelled as literals exactly as `tests/functional/phases/08-bugs.sh:12` does — the
harness defines no `$PRODUCT`/`$COMPONENT` global.

**Verification.**
- A real Bugzilla round-trip puts no raw control or bidi character on the terminal.
  `Mode: focused-test`. The phase's four cases. Red against a build without Tasks
  2–4: `assert_stdout_not_contains` fails on the raw character. Green:
  `make functional-test`.
- `docs/bzr-cli.md` records the rendering change. `Mode: task-test-not-applicable`.
  Prose for a human reader; `tools/` contains no doc-content check that reads its
  body, so no executable or structural observation over it could fail meaningfully.

1. Write the phase with ids `escape-summary-bidi`, `escape-summary-esc`,
   `escape-comment-body`, `escape-json-passthrough`:

   ```bash
   PAYLOAD_RLO=$'‮'
   PAYLOAD_ESC=$'[2J'

   test_begin "escape-summary-bidi" "bug view escapes a bidi override in the summary"
   run_bzr bug create --product FuncTestProd --component Backend \
       --summary "escape probe ${PAYLOAD_RLO} tail" --description "probe"
   if assert_success; then
       ESCAPE_BUG=$(jq -r '.id' "$BZR_STDOUT")
       run_bzr bug view "$ESCAPE_BUG" --fields summary
       if ! jq -r '.summary' "$BZR_STDOUT" | grep -q "$PAYLOAD_RLO"; then
           test_skip "server did not store the bidi payload"
       else
           run_bzr_raw bug view "$ESCAPE_BUG"
           if assert_success &&
               assert_stdout_not_contains "$PAYLOAD_RLO" &&
               assert_stdout_contains '\u{202e}'; then
               test_pass
           fi
       fi
   fi
   ```

   The round-trip guard is deliberate: if the container's Bugzilla normalises the
   payload away, the case skips rather than passing vacuously or failing on server
   behaviour bzr does not control. Repeat that shape for `PAYLOAD_ESC` in a summary,
   for a comment body via `comment add` / `comment list`, and for a case asserting
   `bzr bug view --json` still carries the raw bidi character — pinning the exclusion
   this change deliberately does not close.
2. Register `08h-terminal-escaping` in `run-tests.sh`'s phase list, immediately after
   `08g-bug-arbitrary-fields`.
3. `make check-shell`, `bash -n tests/functional/phases/08h-terminal-escaping.sh`, and
   `tools/check-functional-test-ids.sh .` — all clean, ids unique.
4. In `docs/bzr-cli.md`, add a paragraph to the output-format section: table output
   escapes C0/C1 controls and Trojan-Source bidi controls in server-supplied values as
   `\u{…}`; `--json`/`--ndjson` are unchanged; a tab inside a comment body now renders
   as `\t`.
5. `make lint`, `make test`, then `make functional-test`; report the Bugzilla version
   that ran.
6. Commit `test(functional): prove terminal escaping against a live server` and
   `docs(output): record the terminal-escaping rendering change`.

**Acceptance.** `make functional-test` green with the new phase running (or skipping with a named server-normalisation reason); `docs/bzr-cli.md` describes the rendering.

## Rollback

Each task is a separate commit touching only `src/output/**`, `docs/**`, and
`tests/functional/**`. `git revert` over the range restores the previous rendering:
no migration, no persisted state, no published-schema change to unwind.
