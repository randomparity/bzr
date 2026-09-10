# Terminal-control escaping at composition sites outside the writers

Issue: [#759](https://github.com/randomparity/bzr/issues/759).
Decision: [ADR 0070](../../adr/0070-escape-terminal-controls-at-composition-sites.md),
extending [ADR 0065](../../adr/0065-escape-terminal-controls-in-every-writer.md).
Plan: `../plans/2026-09-10-terminal-escaping-composition-sites.md`.

## Problem

ADR 0065 (#743) made every writer under `src/output/**` escape Unicode `Cc` plus the
Trojan-Source bidi set before server text reaches the terminal. It deliberately left three
bypasses and recorded them as follow-up, because their inputs are composed outside
`src/output/**` or their seam cannot escape without breaking a caller:

- The batch command layer writes a per-item server error to stderr directly
  (`src/commands/bug/update/output.rs:37`, `src/commands/bug/create_json.rs:322`). The
  per-item error is a `BzrError` display whose `Api`/`HttpStatus` variants embed
  server-supplied text.
- `src/main.rs:50` (the `resolve_format` failure arm) and the `format_dispatch_error` Table
  arm (the `dispatch` failure path) print `error: {e}` — a `BzrError` display that can carry
  a server message.
- `src/output/result_types.rs`'s `write_result` table arm prints a `human_message` composed in
  `src/commands/**`; `src/commands/schema.rs:112` passes `names.join("\n")` and depends on the
  embedded newlines, so the seam itself must not escape.

`escape_terminal_controls` is `pub(super)`, so none of these sites can reach the helper. A
hostile or compromised Bugzilla can still plant a raw ESC or a bidi override in a per-item
error or a dispatch error and have it reach the terminal unescaped.

## Architecture

ADR 0070 holds the decision; this section states what the implementation must satisfy.

`escape_terminal_controls(&str) -> String` (`src/output/formatting.rs`) keeps its predicate
(`char::is_control()` plus the Trojan-Source bidi set) and its `escape_default()` spelling;
only its visibility widens from `pub(super)` to `pub` so `src/commands/**` and the binary
reach it.

Each bypassing sink escapes its own server-controlled interpolation at the composition point:

- The two batch stderr lines wrap the per-item `f.error` in `escape_terminal_controls`.
- The two `src/main.rs` `error: {…}` table renderings wrap the `BzrError` display in
  `escape_terminal_controls`. The JSON/NDJSON arms of `format_dispatch_error` are unchanged —
  they route through `serde_json`.
- `write_result`/`write_saved` are left unescaped at the seam. `src/commands/schema.rs`
  passes static names, so its (no-op) escaping is not added; a doc comment on `write_result`
  records the "print composed string, callers escape their own interpolations" contract.

## Scope

In: `src/output/formatting.rs` (visibility + doc), `src/commands/bug/update/output.rs`,
`src/commands/bug/create_json.rs`, `src/main.rs` (both `error: {…}` table renderings),
`src/output/result_types.rs` (doc comment only), and their test siblings
(`update/output_tests.rs`, the `create_json` test sibling, `main_tests.rs`), one new
functional phase under `tests/functional/phases/`, the `docs/adr/0070-*` record, its
`docs/adr/README.md` index row (this is a solo run, and the index is not CI-gated), and
`docs/bzr-cli.md`.

Out, each with an owner:

- The `--json`/`--output ndjson` family — published schema surface; ADR 0065 records the
  JSON-family bidi gap as a separate follow-up. [owner: ADR 0065]
- Seam-level escaping of `write_result`/`write_saved` — would collapse
  `src/commands/schema.rs`'s `names.join("\n")` listing. [owner: ADR 0065 + this record]
- `write_result`/`write_saved` callers in other files that pass server-controlled
  `human_message` (e.g. comment-tag listings in `src/commands/comment/`) — same defect class,
  not a site this follow-up was filed to close. [owner: adjacent; tracked separately]
- The `Cf` code points outside the Trojan-Source set — ADR 0065 follow-up. [owner: ADR 0065]

## Threat model

**Boundary inventory.** The same boundary ADR 0065 named — the Bugzilla REST/XML-RPC response
body, deserialized into `src/types/**` and rendered onto the terminal — is unchanged. This
change adds no boundary; it adds the missing control on the three sinks ADR 0065 left open:
the batch stderr lines, the two `src/main.rs` `error: {…}` renderings, and (by contract) the
`write_result`/`write_saved` table arms, whose callers now own their interpolations. The
server-supplied text enters through `BzrError::Api { message }` and
`BzrError::HttpStatus { body }` (`src/error.rs`), which are redacted for API keys but not for
terminal controls.

**Actor model.** The untrusted party is the configured Bugzilla server — hostile,
compromised, or relaying attacker-supplied values. A per-item error body or a dispatch error
message is produced by the server, so a raw ESC or bidi override in it reaches the local
terminal. bzr trusts the local config file and the user's CLI arguments (operator-controlled);
the `src/commands/schema.rs` listing is static names, so it is not an untrusted input.

**Control per boundary.** Destination encoding at the composition point:
`escape_terminal_controls` wraps each server-controlled interpolation before it is written.
It fails open in one direction only (a character outside the predicate renders verbatim),
which is why ADR 0065 states the predicate. Nothing is logged on failure; the function cannot
fail.

**Out of scope.** Escape sequences bzr itself emits (`colored`) are trusted and never
escaped. The JSON/NDJSON family is not encoded against bidi. `U+200B`/`U+200C`/`U+200D`/
`U+FEFF` remain permitted (ADR 0065). Response-body size bounding is a separate concern.

## Success

1. `escape_terminal_controls` is reachable from `src/commands/**` and `src/main.rs`
   (visibility `pub`); its predicate and spelling are unchanged.
2. The batch update and batch create stderr failure lines render `\u{…}` for any `Cc`/bidi
   character in the per-item error, and leave an ASCII error byte-identical.
3. The `src/main.rs` `error: {…}` table rendering escapes a `BzrError` whose display carries
   a `Cc`/bidi character; the JSON and NDJSON arms of the dispatch error are byte-identical to
   before (still `serde_json`).
4. `write_result`'s table arm is unchanged: `src/commands/schema.rs`'s
   `names.join("\n")` listing still prints one name per line (no literal `\n`).
5. `--json` and `--output ndjson` output for every changed path is byte-identical to before.
6. `docs/bzr-cli.md` records the stderr rendering change; the ADR 0070 index row is present.

## Validation

Every entry is `Mode: focused-test` unless marked otherwise; the plan carries each test's red
observation and exact green command.

- **Visibility + predicate unchanged** (success 1) — the existing
  `formatting_tests.rs::escape_terminal_controls_escapes_cc_and_bidi_only` still passes
  unchanged after the visibility edit; the command-site tests below compile only if the
  helper is reachable, which is the compile-time proof of the widening.
- **Batch update stderr escape** (success 2) — `update/output_tests.rs`: a `BatchResult` with
  a `BatchFailure` whose `error` carries `\u{1b}[2J` and `\u{202e}` renders both escaped in the
  captured stderr; an ASCII-only error is byte-identical.
- **Batch create stderr escape** (success 2) — the `create_json` test sibling: a
  `BatchCreateResult` with a `CreateFailure` whose `error` carries the same payload renders it
  escaped in the captured stderr.
- **Dispatch error table arm** (success 3) — `main_tests.rs`: `format_dispatch_error` for a
  `BzrError::Api` whose `message` carries `\u{1b}` and `\u{202e}` renders them escaped in the
  `Table` arm, while the `Json` and `Ndjson` arms are unchanged (the payload reaches
  `serde_json` raw, so the JSON arm still carries the raw bidi character).
- **`resolve_format` error arm** (success 3) — `Mode: task-test-not-applicable`. The arm is
  inline in `main()` (not a callable function) and renders `resolve_format`'s local validation
  errors, which carry no server-controlled text; the change is a one-line wrap in the
  already-tested `escape_terminal_controls`, and the dispatch arm above — the site where server
  text actually flows — carries the focused test.
- **`write_result` seam unchanged** (success 4) — `result_types_tests.rs`:
  `write_result` in the table arm prints a `human_message` containing `\n` verbatim (one line
  per `\n`), proving the seam did not start escaping.
- **End-to-end** (success 2, 3) — `tests/functional/phases/08i-batch-error-escaping.sh`: a
  real batch update where one item fails; assert the `Failed to update bug #…:` line is
  present, the exit code is the batch-partial-failure code, and a hostile per-item error (when
  the server round-trips it) is escaped rather than raw. A round-trip probe skips the hostile
  assertion with a named skip when the container normalises the payload away.
- **`docs/bzr-cli.md` and the ADR 0070 index row** (success 6) —
  `Mode: task-test-not-applicable`. Both are prose whose only consumer is a human reader; the
  repository has no doc-content gate that reads either file's body, so no executable or
  structural observation over them could fail meaningfully.
