# ADR 0065: One terminal-escaping policy for every table and detail writer

## Status

Accepted

## Context

`escape_table_control` (`src/output/formatting.rs`) filters on `char::is_control()` —
Unicode category `Cc` and nothing else — and has exactly two callers on `main`:
`src/output/resources/field.rs:71` and `src/output/resources/comment.rs:40`. Every
other writer under `src/output/` escapes nothing. `bug list` builds its own
`tabled::Builder` and pushes server strings straight in; `bug view` interpolates
`bug.summary` into a `writeln!`; `comment list` prints bodies line by line; the
product, classification, component, group, user, server, and attachment writers all
interpolate server strings directly. So a raw ESC from a hostile or compromised
Bugzilla already reaches the terminal today, and the bidi overrides ADR 0062 recorded
as a residual reach it through every writer, not only the two that escape.

Two things make this one decision rather than a dozen. The gap is repo-wide, so
fixing it one call site at a time reproduces the divergence ADR 0062 declined to
introduce. And the categories are not separable: a helper that already visits every
character to escape `Cc` widens to the bidi set by changing one predicate.

ADR 0062 also states, in its consequences and in the doc comment on
`escape_table_control`, that "serde escapes control characters when it serializes".
That holds for `Cc` only: `serde_json` escapes `"`, `\`, and code points below
`0x20`. Bidi overrides pass through `--json` and `--output ndjson` verbatim.

## Decision

**One predicate: `Cc` plus the Trojan-Source bidi set** — `char::is_control()`
unchanged, plus `U+202A`–`U+202E`, `U+2066`–`U+2069`, `U+200E`, `U+200F`, `U+061C`.
That is what rustc's `text_direction_codepoint_in_literal` lint covers for
CVE-2021-42574, and it is where the line is defensible: these code points reorder
rendered text invisibly, so escaping them costs a reader nothing.

**The rest of `Cf` stays.** `U+200C`/`U+200D` (ZWNJ, ZWJ) are load-bearing for
Persian and Hindi orthography and for emoji sequences; escaping them corrupts
legitimate text. `U+200B` and `U+FEFF` are invisible but do not reorder. Widening to
all of `Cf` is a separate decision with a different cost.

**Escape, not strip or replace.** `char::escape_default()` renders both halves
correctly — `\u{202e}`, `\u{1b}` — and the two existing callers already produce that
spelling with tests pinning it. Stripping drops content silently; U+FFFD loses which
character was removed. Only characters the predicate selects reach `escape_default`,
so `é` stays `é` rather than becoming `\u{e9}`.

**Apply it at the shared seams, and remove the bypass.** `write_table_records`
escapes every header and cell, so `write_records_or_empty` and its six resource
callers inherit it. `write_field`, `write_optional_field`, and `write_list_field`
escape labels and values, covering the detail views. `write_table`, which takes an
already-built `Table` and cannot see cells, is folded into `write_table_records`, and
`src/output/resources/bug.rs`'s four `Builder::default()` sites move onto the seam.
Leaving `write_table` reachable would leave a way to build a table that escapes
nothing, which is how the current gap arose.

**Sites that compose their own line escape their own interpolations.** The `bold()`
headers, `bug history`, `comment list` bodies, the attachment batch listing, the
product and classification detail blocks, and `server capabilities` pass through
neither seam; they call the helper per interpolation, and a per-writer unit case is
what catches a missed one.

**Truncate first, then escape.** `truncate` counts `chars` while
`tabled::Width::wrap` measures display width. Escaping first would let an
eight-character escape be cut in half and would spend the column budget on escapes;
escaping last leaves the record closures unchanged and yields a pure-ASCII cell whose
display width equals its character count — the only form `Width::wrap` measures
correctly.

**Colour is applied after escaping, never escaped.** `colorize_status` is the one
value reaching `write_field` already carrying bzr's own ANSI. It moves to a dedicated
`write_status_field` seam that escapes the server's status text and colours the
result, so the only ESC bytes emitted are the ones bzr chose.

**The JSON family is unchanged, and the false claim is removed.** `--json` and
`--output ndjson` are a published schema surface; escaping bidi there is a contract change,
not a rendering fix. The doc comment and this record state what serde actually
escapes; the JSON-family bidi gap is recorded as follow-up.

## Consequences

- **Output changes for any value carrying these code points.** A field name, comment
  tag, summary, or real name containing ESC or a bidi override now renders `\u{…}`,
  so a script matching on such a value sees the escaped spelling. That is the point,
  and `docs/bzr-cli.md` records it.
- **A tab in a comment body now renders as `\t`.** `\t` is `Cc`, one predicate serves
  both prose and cells, and comment bodies frequently carry pasted logs. Exempting
  `\t` would mean two predicates — a raw tab breaks a table frame — which is the
  thing this record exists to avoid. Visible and accepted.
- **An escaped cell is wider than its truncation width.** A 72-character summary with
  five overrides becomes 107 characters and wraps under `--width`. Wrapping
  attacker-controlled content is correct; hiding it inside a reordering column is
  not. No truncation width changes.
- **`write_result` is deliberately not escaped.** Its table arm prints a message
  composed in `src/commands/**`, and `src/commands/schema.rs:112` passes
  `names.join("\n")` — escaping would collapse that listing to one line with literal
  `\n`. That seam and the `error: {e}` line at `src/main.rs:50` are outside this
  record's surface and are recorded as follow-up.
- **`escape_table_control` becomes `escape_terminal_controls`.** It is no longer
  table-specific, and a name saying "table" is what let the detail views go
  unescaped. Crate-internal, so no published surface changes.
- **ADR 0062's residual is closed and its serde claim corrected** by a one-line
  amendment on that record pointing here. Its decision is untouched: the residual was
  explicitly scoped to "every table writer at once", which is this.

## Considered & rejected

- **Escape all of Unicode `Cf`.** verified: `UnicodeData.txt` (Unicode Standard,
  UAX #44) gives `U+200C ZERO WIDTH NON-JOINER` and `U+200D ZERO WIDTH JOINER` the
  category `Cf`, and UAX #31 and UTS #51 require both for Persian ZWNJ forms and ZWJ
  emoji sequences. judgment: corrupting correct text for every RTL and emoji user
  buys no display-integrity gain, since neither code point reorders.
- **A conservative allowlist.** judgment: the permitted set is "most of Unicode", so
  the allowlist is a denylist written inside out, and every script bzr has not
  enumerated renders as escapes.
- **Strip instead of escape.** verified:
  `write_field_names_table_escapes_control_characters`
  (`src/output/resources/field_tests.rs`) and
  `write_comments_table_escapes_tag_controls`
  (`src/output/resources/comment_tests.rs`) both assert the `\u{1b}` spelling today.
  judgment: stripping also makes a forged row and an honest one look identical.
- **Sanitize at the `Writers` boundary so nothing can bypass it.** verified:
  `colored` emits real ESC sequences, and `colorize_status`
  (`src/output/formatting.rs:292`), `src/output/resources/comment.rs:28`, and every
  `.bold()` header line produce them on the same stream. judgment: a filter there
  would escape bzr's own colour codes, so the boundary that looks safest is the one
  place the policy cannot live.
- **A `Sanitized<'a>(&'a str)` newtype implementing `Display`.** judgment: it reads
  well at the bespoke `writeln!` sites and nowhere else — the seams take `String`
  cells and the coloured headers need an owned value for `.bold()` — so half the call
  sites would unwrap it immediately.
- **Escape inside `write_table` rather than `write_table_records`.** verified:
  `write_table` receives a built `tabled::Table` whose cells are no longer
  addressable as strings. judgment: not implementable at that layer.
- **Escape in `write_result` too.** verified: `src/commands/schema.rs:112` passes
  `available_names().join("\n")` as the human message. judgment: the seam's contract
  is "print this composed string", its inputs are built outside `src/output/`, and a
  caller depends on embedded newlines.
- **Escape bidi in `--json`/`--output ndjson`.** verified: the envelope is published
  (`schemas/*.json`; `SCHEMA_VERSION` is `3.0.5` at `src/output/mod.rs:10`) and
  `serde_json` escapes only `"`, `\`, and code points below `0x20`, so any bidi
  escape would be a non-standard JSON string encoding. judgment: a contract change
  belongs in its own record with its own version bump.
- **Do nothing.** verified: the residual is recorded in ADR 0062's consequences and
  in the doc comment on `escape_table_control`, and issue #743 exists to close it.
