# ADR 0070: Escape terminal controls at message-composition sites outside the writers

## Status

Accepted

## Context

ADR 0065 routed terminal-control escaping through the writers in `src/output/**`: one
predicate, `escape_terminal_controls`, applied at the shared seams (`write_table_records`,
the `write_field` family, `write_status_field`) and by an explicit call at each writer that
composes its own line. That closed the writer surface. ADR 0065 explicitly recorded two
bypasses as follow-up ("outside this record's surface") — the `write_result` seam and the
`src/main.rs:50` `error: {e}` line — while the batch command-layer stderr lines sat outside
its `src/output/**` scope. The three:

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
terminal before, or without, passing through a seam that escapes. Auditing the callers of
those seams for this change found further instances of the same bypass that issue #759 did
not name — the `attachment upload` batch's per-item stderr arms, and the sub-step warnings
in `bug compound`, `bug update`, `bug clone`, `bug history` and `comment list` — so the
surface this record closes is wider than the three. `escape_terminal_controls`
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

**A multi-line composed message is escaped line by line, not as one string.** Several
`BzrError` displays end in a bzr-authored remediation hint that is deliberately
multi-line and indented: `tls::error::TLS_HINT` (appended to `BzrError::Http`), the
"TLS certificate not trusted" body in
`commands::runtime::shared::connection::tls_trust`, and the ISO-8601 flag rejection in
`validation::datetime`. `escape_default()` escapes `\n`, so escaping such a display in
one call renders the hint as a single line of literal `\n` — the actionable remediation
text for the most common first-run failure, destroyed. The line structure is bzr's own
and is not server-controlled, so `src/main.rs`'s `format_table_error` splits on `\n`,
escapes each line, and rejoins. The same reasoning applies per interpolation to a
listing composed from many server values (`comment search-tags`): escape each value, not
the joined string.

**The sites closed by this record:**

- `src/commands/bug/update/output.rs` — the per-item `f.error` in the
  "Failed to update bug #…: …" stderr line.
- `src/commands/bug/create_json.rs` — the per-item `f.error` in the
  "Failed to create bug (item …): …" stderr line.
- `src/main.rs` — the `BzrError` at both `error: {…}` table renderings: the
  `resolve_format` failure arm, and the `format_dispatch_error` Table arm (the dispatch path,
  where `Api`/`HttpStatus` server text actually flows).
- `src/commands/comment/tag.rs` and `src/commands/comment/search_tags.rs` — the
  server-echoed tag text composed into the `write_result` table message.
- `src/commands/attachment/download.rs` — the single-download destination in the
  `write_result` table message. Without `--out` it is the server's `file_name` reduced by
  `safe_basename`, which rejects traversal but not control characters. The escape is
  display-only: `DownloadResult.file` keeps the real path, because it is the published
  `--json` schema and names the file that was actually written.
- `src/commands/attachment/upload.rs` — the per-item `f.error` stderr arms of the upload
  batch and the `warn_partial` privacy-flip warning. The plain-failure and
  `comment_private` arms carry a server-supplied `BzrError` display, the same shape as the
  `bug update` / `bug create` batch lines above; the `not_attempted` arm's text is
  bzr-composed, and escaping it through the shared binding is a no-op rather than a second
  code path.
- The command-layer sub-step warnings that print a `BzrError` from a failed API call
  straight to stderr: `src/commands/bug/compound.rs` (comment-tag, comment and attachment
  sub-steps), `src/commands/bug/update/execute.rs` (`warn_comment_tags_failed`),
  `src/commands/bug/clone.rs` (the "Cloned from bug #…" comment), `src/commands/bug/history.rs`
  (the comment-correlation fallback), and `src/commands/comment/list.rs` (the per-bug
  permissive skip line).

These last two groups were not named by issue #759 — they were found auditing the
`write_result` and stderr callers for this change. They share the verified root cause and
the same one-line fix, so they are closed here rather than deferred: a record that leaves
live instances of the defect it describes is not a closed record.

**Local-only error text is left alone.** `src/commands/bug/view.rs`'s "failed to open
browser" and `src/commands/config/keyring.rs`'s config-validation warning interpolate errors
that never carry server data, and ADR 0065's threat model is a hostile or compromised
Bugzilla. Escaping them would add noise without removing a bypass.

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
- **A dispatch `BzrError` is escaped line by line in table mode.** Its static prefix
  ("Bugzilla API error: … (code N)", "HTTP N: ") is ASCII and unaffected; only the
  server-supplied portion renders escaped, and bzr's own multi-line remediation hints keep
  their lines and indentation.
- **A server message that embeds its own newline renders as an extra stderr line.** The
  per-line split trades that for the hints. It is the accepted residual: every `Cc`/bidi
  character on each of those lines is still escaped, and a free-form `error: …` rendering
  has no row structure for an injected line to forge — unlike a table, where ADR 0065
  escapes `\n` precisely because a break would forge a row. `--json` is the surface to
  match on.
- **The `write_result`/`write_saved` table arms remain "print composed string" seams.** Any
  caller that composes server-controlled data into a `human_message` must escape its own
  interpolations; the widened helper makes that possible, and the three callers that carry
  server-controlled data now do. `docs/bzr-cli.md` records the stderr rendering change.

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
  {} (code {code})"` and `"HTTP {status}: {}"` in `src/error.rs` — so escaping is a no-op on
  them; but `Http`, `Config` and `InputValidation` carry static multi-line *bodies*, not just
  prefixes, which a whole-string escape does destroy. That is what the per-line split above
  fixes. judgment: decomposing the `BzrError` to isolate its server-controlled component
  would mean classifying all 20 variants in the caller and re-classifying every variant added
  later — a per-variant audit that rots — to buy only the residual the per-line split accepts.
  The per-line unit gets the hints back for two lines of code.
- **Escape at the `write_result`/`write_saved` seam.** verified:
  `src/commands/schema.rs:112` passes `available_names().join("\n")` and the table arm prints
  it verbatim; escaping the composed string turns each embedded newline into a literal `\n`
  and collapses the multi-line listing to one line. judgment: the seam's contract is "print
  this composed string"; its callers own their interpolations. This is ADR 0065's own
  rejection, restated for the two arms it names.
- **Document the `write_result` caller contract but leave the callers unescaped, tracking
  them separately.** verified: `comment tag`, `comment search-tags` and the single-attachment
  `attachment download` message are the `write_result` callers that interpolate
  server-controlled data, and issue #759 names the `write_result` table arm as one of its
  three sites. judgment: a contract with no caller honoring it and no gate enforcing it is
  not a closed bypass — closing #759 while `bzr comment tag` still puts raw ESC on the
  terminal would be false. The three are one line each with the widened helper, so they are
  in scope. The remaining `write_result` callers pass bzr-composed or operator-supplied text
  (ids, counts, the product/component/user/group names the caller typed, static schema names)
  and are left alone — the operator's own terminal is not the threat actor.
