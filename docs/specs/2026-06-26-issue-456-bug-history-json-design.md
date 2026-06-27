# Issue #456 — structured `--json` mode for `bug history`

- Status: Draft
- Date: 2026-06-26
- Issue: #456
- ADR: [0008](../adr/0008-bug-history-flattened-change-records.md)

## Problem

`bzr bug history <id>` already accepts `--json`, `--output ndjson`, and
`--since`. But the current `--json` output serializes the raw, **grouped**
Bugzilla shape — one object per history entry with a nested `changes` array:

```json
{"who": "alice@example.com", "when": "2026-06-01T14:22:01Z",
 "changes": [{"field_name": "status", "removed": "NEW", "added": "ASSIGNED",
              "attachment_id": null}]}
```

Agents cannot use this for the issue's stated goal — programmatic mid-air
collision detection and "what changed since I last looked" — without
re-implementing the grouping/flattening themselves, per field, per entry. The
`bzr-triage-bug` skill's "read before write" rule has no machine-readable delta
to compare against.

## Goal

Emit one **flattened change record per changed field** in `--json` and
`--output ndjson`, matching the shape documented in the issue:

```json
{"when": "2026-06-01T14:22:01Z", "who": "alice@example.com", "field": "status",
 "old_value": "NEW", "new_value": "ASSIGNED", "comment_id": null}
```

Publish a `history` JSON Schema for the shape. Leave the human-readable table
output and its exit codes byte-for-byte unchanged.

## Scope

In scope:

- Reshape the `--json` / `--output ndjson` body of `bug history` to flattened
  change records.
- Add a `comment_id` field, correlated from the bug's comments (see ADR 0008).
- Publish `schemas/history.json`; register it in the `schema` command.
- Wiremock coverage; functional-test coverage; docs.

Out of scope:

- The table renderer (unchanged).
- `--since` validation and the exit-7 behavior (already implemented and tested).
- Adding new CLI flags (`--json`, `--output`, `--since` already exist).

## Record shape

| key | source | type | notes |
|-----|--------|------|-------|
| `when` | `HistoryEntry.when` | string | ISO 8601, verbatim from the server |
| `who` | `HistoryEntry.who` | string | the user who made the change |
| `field` | `FieldChange.field_name` | string | e.g. `status`, `assigned_to`, `cf_*` |
| `old_value` | `FieldChange.removed` | string \| null | empty string when nothing was removed; null when the server omitted the value |
| `new_value` | `FieldChange.added` | string \| null | empty string when nothing was added; null when the server omitted the value |
| `comment_id` | correlated (ADR 0008) | integer \| null | the comment posted in the same entry, else null |

All six keys are always present (closed schema, `additionalProperties: false`).
`old_value`/`new_value` preserve omitted server values as null instead of
defaulting them to empty strings.

The wire field `attachment_id` is **not** carried into the JSON record (the
issue's documented shape omits it). It remains visible in the table renderer as
the existing `[attachment #N]` suffix. See ADR 0008 "Considered & rejected".

### Multi-field expansion

A single history entry with N changed fields produces N records, each sharing
that entry's `when`/`who`/`comment_id`. This is the core behavioral change and
is the acceptance-criteria wiremock assertion.

## `comment_id` correlation (ADR 0008)

The history REST endpoint carries no comment association. To populate
`comment_id`, when (and only when) the output format is in the JSON family, the
command additionally fetches the bug's comments and matches each history entry
to a comment by `who == creator` AND `when ≡ creation_time`, where `≡` compares
the canonical timestamp key (`validation::datetime::timestamp_compare_key`,
`YYYYMMDDHHMMSS`). On a match, every record from that entry carries the
comment's `id`; otherwise `comment_id` is null.

- The comment fetch happens only for the JSON family — table output makes one
  API call as before.
- **The comment fetch is unfiltered (`since = None`) even when the user passed
  `--since`.** `--since` filters only which *history records* are emitted (the
  server applies `new_since` to the history call). Correlation must join over
  the full comment set: a comment can sit inside the history window but be
  excluded by a `--since` bound on the comment call, which would null a
  `comment_id` that should have matched. The two lists are joined first; the
  `--since` bound is the server's filter on the history side only.
- A comment fetch failure is **non-fatal**: the command warns on stderr and
  emits records with `comment_id: null` (the history delta is the contract;
  comment correlation is best-effort enrichment). See ADR 0008.
- Duplicate `(who, when-key)` comments (rare; multiple comments in one second by
  one user) resolve to the first by ascending comment id; documented, not an
  error.

### Correlation is best-effort and never produces a wrong id

The join key is exact `who == creator` plus a canonical timestamp-key match.
Both inputs come from separate REST resources, so a match can be *missed*
(yielding `comment_id: null`) but never *wrong*. Known miss cases, all
acceptable degradation:

- **User-string skew.** `who` (history) and `creator` (comment) are exact-string
  compared. If the server renders the same user as a login name on one endpoint
  and an email on the other, the join fails and `comment_id` is null.
- **Unkeyable timestamps.** `timestamp_compare_key` returns `None` for
  offset-bearing forms (`+01:00`); such entries cannot correlate.
- **Private/filtered comments.** A REST comment fetch may omit private comments
  (issue #125); a correlating private comment is then invisible and `comment_id`
  is null.

## Empty / edge behavior

- **Table, empty history:** unchanged — prints `No history for bug #<id>.`
- **JSON, empty history:** emits the envelope with an empty array
  (`{"schema_version": "...", "data": []}`), not the prose message — an agent
  parsing JSON must always receive valid JSON.
- **NDJSON, empty history:** emits nothing (zero lines), consistent with the
  existing `write_ndjson` contract for empty slices.
- **`--since` filtering:** unchanged; the server applies `new_since`. Invalid
  `--since` exits 7 before any network I/O (already covered).

## Acceptance criteria (from the issue)

1. `bug history <id> --json` emits an array of change records of the documented
   shape. — flatten + reshape.
2. `--output ndjson` streams one record per line. — same flattening, ndjson sink.
3. `--since <ts>` filters; invalid date exits 7. — already implemented; assert it
   still holds for the JSON path.
4. Multi-field entries → one record per field. — wiremock test.
5. Table output and exit code unchanged for non-JSON. — table path untouched;
   existing tests stay green.
6. `bzr schema` publishes a schema for the shape. — `schemas/history.json`.
7. Wiremock test: multi-field change expands to multiple records. — new test.
8. `docs/bzr-cli.md` updated; `make skills-test` drift check passes. — docs +
   no new flags so `commands.yml` is already current.

## Verification

- Unit/wiremock: multi-field expansion; comment_id correlation hit and miss;
  empty-history JSON emits `[]`; ndjson one-record-per-line; comment-fetch
  failure degrades to null + stderr warning; table output unchanged.
- Schema drift test: a maximally-populated `HistoryRecord` conforms to
  `schemas/history.json`.
- Functional: a phase script runs `bug history <id> --json` against a real
  container and asserts the flattened shape (and the credentialless path).
- Guardrails: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  `make check-test-layout`, `make skills-test`.
