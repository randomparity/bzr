# Terminal-control escaping across every output writer

Issue: [#743](https://github.com/randomparity/bzr/issues/743).
Decision: [ADR 0065](../../adr/0065-escape-terminal-controls-in-every-writer.md),
amending [ADR 0062](../../adr/0062-field-list-enumerates-the-accepted-write-field-set.md).
Plan: `../plans/2026-09-07-terminal-control-escaping.md`.

## Problem

`escape_table_control` (`src/output/formatting.rs`) escapes Unicode `Cc` only, and
only two writers call it (`src/output/resources/field.rs:71`,
`src/output/resources/comment.rs:40`). Every other writer under `src/output/`
interpolates server strings into terminal output unescaped, so a raw ESC — not just
the bidi overrides the issue names — already reaches the terminal from `bug list`,
`bug view`, `bug history`, `comment list`, `product`, `classification`, `component`,
`group`, `user`, `server capabilities`, and the attachment writers.

## Architecture

ADR 0065 holds the decision and its alternatives; this section states only what the
implementation must satisfy.

`escape_terminal_controls(&str) -> String` (renamed from `escape_table_control`)
escapes `char::is_control()` plus `U+202A`–`U+202E`, `U+2066`–`U+2069`, `U+200E`,
`U+200F`, `U+061C`, via `char::escape_default()`; everything else passes through.

It is applied at three seams — `write_table_records` (headers and cells, with
`write_table` folded into it so no escape-free table entry point survives),
the `write_field` family (labels and values), and a new `write_status_field` that
escapes before colouring — and by an explicit call at each writer composing its own
`writeln!`. `src/output/resources/bug.rs`'s four `Builder::default()` sites migrate
onto the table seam. Truncation stays inside the record closures, so the seam escapes
last and an escaped cell is pure ASCII whose display width equals its character count.

## Scope

In: `src/output/formatting.rs`, every `src/output/resources/*.rs`, their `*_tests.rs`
siblings, `docs/adr/0065-*.md`, the one-line ADR 0062 amendment, `docs/bzr-cli.md`,
one functional phase script.

Out, each with an owner:

- `--json`/`--output ndjson` **encoding** — published schema surface; follow-up issue.
- `Cf` outside the Trojan-Source set (`U+200B`, `U+200C`, `U+200D`, `U+FEFF`) —
  follow-up issue.
- Width and wrapping beyond keeping columns correct — ADR 0047.
- Bounded response-body reads — issue #740.
- `attachment get` file content on disk — not terminal rendering.
- `write_result`'s table arm and `src/main.rs`'s `error: {e}` line — inputs composed
  outside `src/output/`, and `src/commands/schema.rs:112` depends on embedded
  newlines; follow-up.

`docs/adr/README.md` is not edited: the index row is appended by the campaign
orchestrator after the wave merges (`index row pending`).

## Threat model

**Boundary inventory.** One boundary, widened by nothing: the Bugzilla REST/XML-RPC
response body, deserialized into `src/types/**` and rendered by `src/output/**` onto
the terminal. The change adds no boundary; it adds the missing control on an existing
one. The JSON family crosses the same boundary and is left as is.

**Actor model.** The untrusted party is the configured Bugzilla server — hostile,
compromised, or merely relaying attacker-supplied values, since a summary, comment
body, comment tag, custom field name, or real name is written by any account that can
file a bug. bzr trusts the local config file and the user's own CLI arguments; both
are under the operator's control and route through the same seams anyway.

**Control per boundary.** Destination encoding at render time, by
`escape_terminal_controls` at the three seams and at each bespoke interpolation. It
fails open in one direction only — a character outside the predicate renders verbatim
— which is why ADR 0065 states the predicate rather than leaving it implicit. Nothing
is logged on failure; the function cannot fail.

**Out of scope.** Escape sequences bzr itself emits (`colored`, `tabled`) are trusted
and never escaped. The JSON family is not encoded against bidi.
`U+200B`/`U+200C`/`U+200D`/`U+FEFF` are permitted. Attachment bytes on disk are not
terminal output. Response-body size bounding is issue #740.

## Success

1. The predicate escapes `Cc` and the Trojan-Source bidi set, leaving `U+200C`,
   `U+200D`, `U+200B`, `U+FEFF`, and ordinary non-ASCII letters untouched.
2. No writer under `src/output/` emits a raw `U+001B` or bidi override from a
   server-controlled value in table mode.
3. `write_table` no longer exists as a separate escape-free entry point.
4. `bug view`'s Status row keeps its colour.
5. Truncation and `--width` wrapping are unchanged for values containing no escaped
   character; `--json` and `--output ndjson` output is byte-identical to before.
6. `docs/bzr-cli.md` records the rendering change, and the false "serde escapes
   control characters" claim is corrected in `src/output/formatting.rs` and in the
   `field_tests.rs` doc comment repeating it.

## Validation

Every entry is `Mode: focused-test` unless marked otherwise; the plan carries each
test's red observation and exact green command.

- **Predicate** (success 1) — `formatting_tests.rs::escape_terminal_controls_escapes_cc_and_bidi_only`.
- **Table seam** (2, 3) — `formatting_tests.rs::write_table_records_escapes_cells_and_headers`.
- **Detail seam** (2) — `formatting_tests.rs::write_field_family_escapes_labels_and_values`.
- **Coloured status row** (4) — `bug_tests.rs::bug_detail_status_escapes_but_keeps_colour`,
  inspecting `ColoredString.fgcolor`, never `colored::control::set_override`.
- **Per-writer coverage** (2) — one `<writer>_table_escapes_terminal_controls` case in
  each of the 14 `*_tests.rs` siblings, feeding `\u{1b}[2J` and `\u{202e}` through
  every table-mode entry point that renders server data.
- **JSON family unchanged** (5) — `formatting_tests.rs::json_family_output_is_not_escaped_for_bidi`.
- **End-to-end** (2) — `tests/functional/phases/08h-terminal-escaping.sh`: a bug and a
  comment carrying `U+001B` and `U+202E`; assert neither raw byte appears in table
  output, both appear escaped, and `--json` still carries the raw bidi character.
- **ADR 0062 amendment and `docs/bzr-cli.md` wording** (6) —
  `Mode: task-test-not-applicable`. Both are prose whose only consumer is a human
  reader; the repository has no doc-content gate that reads either file's body, so no
  executable or structural observation over them could fail meaningfully.
