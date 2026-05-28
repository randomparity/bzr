# `--json` field selection trims the output object

**Date:** 2026-05-27
**Issue:** follow-up on [#206](https://github.com/randomparity/bzr/issues/206) (`fix/issue-206`)
**Supersedes:** the `--json` field advisory shipped earlier on this branch
(commits `518e29f`..`8831cdc`)

## Goal

Make `--fields` / `--exclude-fields` actually trim the JSON output object
to the selected fields, the way `gh ... --json <fields>` does. Today the
`Bug` struct is serialized whole, so a field selection only controls which
values are *fetched* — unselected keys are still emitted with `null`/empty
values. That is a footgun for scripted and agent consumers and wastes
tokens: an agent that asks for two fields should get two fields.

After this change, `bzr bug list --fields summary --json` returns
`[{"summary": "..."}]`, not the full object with everything-but-summary
nulled.

## Decisions (locked during brainstorming)

1. **`id` contract: honor the selection literally (gh-style).** Output
   exactly the requested keys. `--fields summary` omits `id` unless asked;
   `--exclude-fields id` drops `id` from output. This reverses the
   "`id` is always present" guarantee the earlier #206 work established —
   that guarantee was only meaningful because output wasn't trimmed.
   The client still *fetches* `id` internally for deserialization (see
   Non-goals); output trimming is independent.
2. **Unknown/unmodeled field tokens mirror table mode.** All-unknown
   include → exit 7 with the existing message; partial-unknown → warn once
   on stderr and project the known fields; an `--exclude-fields` that drops
   every key → exit 7. The column registry is 1:1 with the `Bug` struct's
   serde keys, so "unknown for a table column" and "unknown for a JSON key"
   are the same set — JSON reuses table mode's validation verbatim.
3. **`bug view` is exempt from the zero-field error in JSON, as it already
   is in table mode.** A sparse or empty single-bug object (`{}`) is a
   coherent result, matching the existing detail-view exemption.
4. **`preserve_order` for `serde_json`.** Enable the feature so projected
   objects keep struct-declaration order rather than going alphabetical,
   keeping trimmed output consistent with the existing full-object output.

## Non-goals

- No change to bare `--json` (no field selection) — it still returns the
  full object.
- No change to table / detail rendering — `field_selected` already controls
  which rows and columns appear.
- No change to `force_id_fields` in the client: `id` is still always added
  to the wire request so every bug deserializes. That is about the request,
  not the output, and is orthogonal to trimming.
- No support for custom `cf_*` fields in JSON output. `Bug` is a fixed
  struct with no `#[serde(flatten)]`, so unmodeled fields are already
  dropped at deserialization and can never appear — trimming does not change
  this. Capturing custom fields is possible future work, out of scope here.
- No new `BzrError` variants. The zero-field cases reuse the existing
  `InputValidation` (exit 7).

## Architecture

### Projection helpers (`src/output/resources/bug.rs`)

```rust
/// Project a bug into a JSON object honoring `spec`:
/// - include: keep exactly the canonical keys named (intersection)
/// - exclude: drop the canonical keys named
/// - neither: full object, unchanged
fn bug_to_json(bug: &Bug, spec: ColumnSpec<'_>) -> serde_json::Value;

/// `bug_to_json` over a slice, for the array output paths.
fn bugs_to_json(bugs: &[Bug], spec: ColumnSpec<'_>) -> Vec<serde_json::Value>;
```

Implementation:
1. `serde_json::to_value(bug)` → a `Value::Object`. With `preserve_order`
   the map keeps struct order.
2. Resolve `spec` to a canonical key set via the existing
   `canonical_field_list` (include) and a `partition_include`-based split
   for the known/unknown distinction — the same primitives table mode uses,
   so alias resolution (`assignee`→`assigned_to`, `platform`→`rep_platform`)
   can't drift between modes.
3. Include: `retain` keys in the canonical set. Exclude: `remove` the
   canonical keys. Neither: return the object untouched.

Projection operates only on keys already present in the serialized object,
so unknown tokens are inert at this layer; the warn/error behavior for them
is handled by validation (below), not here.

### Validation becomes mode-agnostic

`validate_table_columns` is renamed `validate_field_selection` and its doc
updated (it is no longer table-only — the known set is identical for both
output modes). Its error messages are genericized off "table column" so they
read correctly in JSON mode too: all-unknown include → "none of the requested
fields are valid bug fields: …"; exclude-everything → "--exclude-fields
removed every field; nothing left to display". The pre-network gate in
`src/commands/bug/mod.rs` and `src/commands/query.rs` collapses to one path:

```rust
if let Some(spec) = bug_column_spec(action) {
    if !matches!(action, BugAction::View { .. }) {
        validate_field_selection(spec)?;   // exit 7 on zero fields, both modes
    }
}
```

The `format == Json` branch that called `warn_json_field_selection` is
removed. `query run` mirrors this (no `match format` split for validation).

### Partial-unknown warning for JSON

Table mode warns about ignored unknown tokens at render time inside
`resolve_columns`. For JSON, the projection path computes the unknown tokens
once (via `partition_include`) and emits the warning on `err` a single time
before projecting — not once per bug. The warning text is genericized to
`"warning: ignoring unknown field(s): …"` (dropping the table-specific
"no table column" phrasing) so it reads correctly in both modes; the existing
table-mode tests that assert the old wording are updated accordingly. None of
the new phrasings collide with the forbidden phrases checked by
`cli_and_docs_avoid_misleading_trim_phrasing`.

### JSON sinks

Three call sites serialize bugs; each switches to projected values:

| Path | Commands | Change |
|------|----------|--------|
| `write_bugs` JSON branch | `bug list`, `bug my`, `bug search`, `query run` | serialize `bugs_to_json(bugs, spec)` |
| `write_bug_detail` JSON branch | single `bug view` | serialize `bug_to_json(bug, spec)` |
| multi `bug view` | `bug view <many>` | emit `{"bugs": [bug_to_json…], "failed": […]}`; wrapper keys and `BugViewFailure {id, error}` are metadata, untrimmed |

`write_formatted`/`write_json` stay generic; the bug writers project before
handing a `Value`/`Vec<Value>` to the JSON path. The multi-bug wrapper is
built explicitly (e.g. `serde_json::json!({...})` or a small struct holding
`Vec<Value>`) since `MultiBugViewResult`'s typed `Vec<Bug>` would serialize
whole.

## Data flow

```
CLI args ──> ColumnSpec ──> validate_field_selection (pre-network, exit 7 on zero fields; view exempt)
                                  │
                          network fetch (client always includes id on the wire)
                                  │
                       bug_to_json / bugs_to_json
                         · resolve aliases -> canonical keys (shared with table mode)
                         · warn once on partial-unknown
                         · include: retain named keys | exclude: drop named keys | none: full
                                  │
                          serde_json -> stdout
```

## Removals (replace, don't deprecate)

- `warn_json_field_selection` and the `json_selection_restricts` helper in
  `src/output/resources/bug.rs`.
- Their unit tests in `src/output/resources/bug_tests.rs`
  (`warn_json_field_selection_*`).
- The #206 CHANGELOG bullet's "null/empty" / advisory language → rewritten
  to describe trimming and the gh-style `id` contract.
- The `--fields` / `--exclude-fields` per-arg help and the `bug view`
  long-doc in `src/cli/bug.rs`, and the matching prose in
  `docs/bzr-cli.md`: replace "unselected fields come back null/empty" with
  "the JSON object contains only the selected fields." The XML-RPC `bug
  view` no-op note narrows to table/detail output, since JSON is now trimmed
  client-side regardless of transport (this closes Finding 3).

## Testing (TDD)

**Unit — `src/output/resources/bug_tests.rs`:**
- `bug_to_json` include keeps exactly the named keys (e.g. `summary,status`
  → object with those two keys only).
- include with alias (`assignee`) yields canonical key (`assigned_to`).
- `--exclude-fields id` drops `id`; `--exclude-fields cc,keywords` drops
  those, keeps the rest.
- no selection (`None`/`""`/`,,`) → full object, all keys present.
- partial-unknown (`summary,cf_x`) → object has `summary`, stderr has the
  one ignore-warning, no `cf_x` key.
- key order matches struct order (locks the `preserve_order` decision).
- `bugs_to_json` projects every element.

**Command/integration:**
- all-unknown include under `--json` → exit 7 (mirrors table); reuse/extend
  the existing zero-column tests now that validation is mode-agnostic.
- multi `bug view --json --fields summary` → each entry in `bugs` trimmed,
  `failed` untouched.
- `cli_and_docs_avoid_misleading_trim_phrasing` stays green after the prose
  rewrites (no forbidden phrase reintroduced).

**Dependency/build:** add `serde_json` `preserve_order`; confirm full-object
output ordering is unchanged and `cargo test`/`clippy`/`fmt` clean.

## Blast radius

- **Output shape changes** for any existing `--json` + field-selection
  invocation: previously a full object with nulls, now a trimmed object.
  This is the intended fix. `jq` on a now-absent key still returns `null`,
  so well-behaved consumers are unaffected; consumers that distinguished
  "key present and null" from "key absent" would see a difference (rare,
  and the prior shape was the bug).
- **`--exclude-fields id --json`** now omits `id` (was: present). Documented
  as part of the gh-style contract.
- **Auto-JSON on a pipe** is unchanged: JSON is still selected when stdout
  is not a TTY. Trimming applies there too, which is the point.
