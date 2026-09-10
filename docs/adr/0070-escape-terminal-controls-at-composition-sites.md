# ADR 0070: Escape terminal controls at message-composition sites outside the writers

## Status

Accepted

## Context

ADR 0065 routed terminal-control escaping through the writers in `src/output/**`: one
predicate, `escape_terminal_controls`, applied at the shared seams (`write_table_records`,
the `write_field` family, `write_status_field`) and by an explicit call at each writer that
composes its own line. That closed the writer surface, but ADR 0065 deliberately left three
sites unescaped and recorded them as follow-up ("outside this record's surface"):

- The batch command layer prints a per-item server error straight to stderr —
  `src/commands/bug/update/output.rs` ("Failed to update bug #…: …") and
  `src/commands/bug/create_json.rs` ("Failed to create bug (item …): …"). The per-item
  error is `e.to_string()` for a `BzrError` whose `Api`/`HttpStatus` variants embed
  server-supplied text.
- `src/main.rs` prints `error: {e}` at process exit. The dispatch path
  (`format_dispatch_error`'s Table arm, reached from `dispatch`'s `Err`) renders a
  `BzrError` whose display can carry a server message; the `resolve_format` failure arm
  renders one too.
- `src/output/result_types.rs`'s `write_result` table arm prints a `human_message` composed
  in `src/commands/**`. `src/commands/schema.rs` passes `available_names().join("\n")` and
  depends on those embedded newlines, so ADR 0065 rejected escaping at that seam.

All three are bypasses of the writer seams: the server-controlled string reaches the
terminal before, or without, passing through a seam that escapes. `escape_terminal_controls`
is `pub(super)` — visible only within `src/output/` — so `src/commands/**` and the binary
(`src/main.rs`) cannot reach it. This record closes the residual ADR 0065 named. It extends,
and does not supersede, ADR 0065: the predicate, its bidi set, its `escape_default()`
spelling, the JSON-family exclusion, and the truncate-first-then-escape ordering are all
unchanged.

## Decision

**Composition sites escape their own interpolations.** A message composed outside
`src/output/**` and printed by a bypassing sink — a direct `writeln!` to stderr, the
`write_result`/`write_saved` table arm, or the `error: {…}` line — escapes each
server-controlled interpolation at the point it is composed, with
`escape_terminal_controls`. This is ADR 0065's own rule ("Sites that compose their own line
escape their own interpolations") applied to the sites ADR 0065 could not reach.

**The helper is exposed at `bzr::output::escape_terminal_controls`.** It was
`pub(super)` in a private module (`mod formatting;`), so neither `src/commands/**` (the
library) nor the binary (`src/main.rs`) could reach it. Making the function `pub` alone is not
enough: the binary reaches the library only through `pub` items, and `formatting` is not one.
The minimal exposure that reaches both without opening the whole module is a single
`pub use formatting::escape_terminal_controls;` re-export in `src/output/mod.rs`, with the
function itself widened to `pub`. Only the reach widens; the function body, predicate, and
`escape_default()` spelling are untouched.

**The `write_result`/`write_saved` table arms are not escaped at the seam.** Their contract
is "print this composed string"; the inputs are built by callers. Escaping the seam would
collapse `src/commands/schema.rs`'s newline-separated listing into one line. Callers that
compose server-controlled data into a `human_message` escape their own interpolations;
`src/commands/schema.rs` passes static schema names, so its escape is a no-op and the seam is
left exactly as ADR 0065 left it.

**The sites closed by this record:**

- `src/commands/bug/update/output.rs` — the per-item `f.error` in the
  "Failed to update bug #…: …" stderr line.
- `src/commands/bug/create_json.rs` — the per-item `f.error` in the
  "Failed to create bug (item …): …" stderr line.
- `src/main.rs` — the `BzrError` at both `error: {…}` table renderings: the
  `resolve_format` failure arm, and the `format_dispatch_error` Table arm (the dispatch path,
  where `Api`/`HttpStatus` server text actually flows).

## Consequences

- **The helper is now a public library function.** This revises ADR 0065's consequence
  "Crate-internal, so no published surface changes": the function is now reachable by the
  binary and by any consumer of the `bzr` crate. It is a pure, well-documented string
  transform with an unambiguous contract, so the added surface is one additive API item; no
  existing published behavior changes. ADR 0065's record is left untouched — this is a new
  record, not an amendment to it.
- **Table-mode stderr for the named sites now renders `\u{…}`** for any `Cc`/bidi character
  in a per-item server error and in a dispatch `BzrError`. A script matching on such a value
  sees the escaped spelling — the same consequence ADR 0065 already accepted for the writers.
- **The JSON/NDJSON family is untouched.** The dispatch-error JSON/NDJSON arms still route the
  message through `serde_json`, which escapes only `"`, `\`, and code points below `0x20`;
  bidi passes through there verbatim, exactly as ADR 0065 left it. Only the table renderings
  change.
- **A dispatch `BzrError` is escaped as a whole string in table mode.** Its static prefix
  ("Bugzilla API error: … (code N)", "HTTP N: ") is ASCII and unaffected; only the
  server-supplied portion renders escaped.
- **The `write_result`/`write_saved` table arms remain "print composed string" seams.** Any
  caller that composes server-controlled data into a `human_message` must escape its own
  interpolations; the widened helper makes that possible. `docs/bzr-cli.md` records the
  stderr rendering change.

## Considered & rejected

- **Keep the helper `pub(crate)` and add a separate public seam for the binary.**
  verified: `src/main.rs` is a separate binary crate that reaches the library only via `pub`
  items (it already calls `bzr::output::progress::error_event` and
  `bzr::error::clear_error_redaction_context`); `pub(crate)` is invisible to it, so a second
  `pub` function would be required to serve the two `src/main.rs` sites. judgment: wrapping an
  already-clean, well-documented pure function in a second public seam adds a surface with its
  own contract to reach the same bytes; exposing the helper directly is the smaller surface.
- **Escape only the server-supplied field of the `BzrError`, not the whole display.**
  verified: the `Api`/`HttpStatus` display prefixes are static ASCII — `"Bugzilla API error:
  {} (code {code})"` and `"HTTP {status}: {}"` in `src/error.rs` — so escaping the whole
  string is a no-op on them. judgment: decomposing the `BzrError` to isolate its
  server-controlled component would duplicate the variant shapes in the caller for no gain;
  the per-interpolation unit at `error: {err}` is the whole display.
- **Escape at the `write_result`/`write_saved` seam.** verified:
  `src/commands/schema.rs:112` passes `available_names().join("\n")` and the table arm prints
  it verbatim; escaping the composed string turns each embedded newline into a literal `\n`
  and collapses the multi-line listing to one line. judgment: the seam's contract is "print
  this composed string"; its callers own their interpolations. This is ADR 0065's own
  rejection, restated for the two arms it names.
- **Widen to `pub` and have every `write_result` caller escape opportunistically (including
  comment-tag listings in other files).** judgment: those callers are the same defect class
  but were not the sites this follow-up was filed to close; they are flagged as adjacent and
  tracked separately, so this change stays the size ADR 0065's "recorded as follow-up" named
  rather than opening the whole `human_message` surface at once.
