# 0008 — `bug history` JSON emits flattened change records with correlated `comment_id`

- Status: Accepted
- Date: 2026-06-26
- Issue: #456

## Context

`bzr bug history <id>` already supports `--json`, `--output ndjson`, and
`--since`. Its JSON body is the raw Bugzilla shape: one object per history entry
with a nested `changes` array (`{who, when, changes:[{field_name, removed,
added, attachment_id}]}`). Issue #456 needs a flat, one-record-per-changed-field
shape so an agent can compute a field-level delta for mid-air-collision
detection without re-grouping:

```json
{"when": "...", "who": "alice", "field": "status",
 "old_value": "NEW", "new_value": "ASSIGNED", "comment_id": null}
```

Two decisions have viable alternatives worth recording: the output shape (and
whether to keep emitting the grouped shape) and how `comment_id` is populated,
given that the Bugzilla history REST endpoint (`GET /rest/bug/<id>/history`)
returns **no comment association at all**.

## Decision

1. **Replace the grouped JSON body with flattened change records.** For the JSON
   family (`--json`, `--output ndjson`), `bug history` emits an array (or
   one-per-line stream) of `HistoryRecord` objects, one per changed field, each
   carrying `when`, `who`, `field`, `old_value`, `new_value`, `comment_id`.
   Records from the same history entry share `when`/`who`/`comment_id`. The
   grouped `{who, when, changes:[...]}` JSON shape is **removed**, not retained
   alongside the new one — there is no released schema for it (the envelope only
   guarantees `schema_version`), and keeping both would be a dual contract.

2. **The table renderer is unchanged.** Table output keeps the grouped,
   colorized rendering (including the `[attachment #N]` suffix) and the
   empty-history `No history for bug #<id>.` line. Only the JSON-family branch
   changes.

3. **`comment_id` is correlated from a second fetch, best-effort.** When the
   format is in the JSON family, the command also fetches the bug's comments and
   matches each history entry to a comment by `who == creator` AND a canonical
   timestamp-key match on `when`/`creation_time`
   (`validation::datetime::timestamp_compare_key`). A match sets `comment_id`
   for every record from that entry; otherwise it is null. The comment fetch is
   **non-fatal**: on failure the command warns on stderr and proceeds with
   `comment_id: null`. Table output makes no extra fetch. The comment fetch is
   **unfiltered** even under `--since`: `--since` constrains which history records
   are emitted, but correlation joins over the full comment set, so a comment
   inside the history window is never excluded by a second filter. The join can
   miss (→ null) but never produce a wrong id (user-string skew, unkeyable
   offset timestamps, REST-filtered private comments).

4. **`comment_id` is the only correlated field; the rest are 1:1 from history.**
   `old_value`/`new_value` are the server's `removed`/`added` strings verbatim
   when present; empty string means Bugzilla reported no value on that side, and
   `null` means the server omitted the value. `attachment_id` from the wire is
   dropped from the JSON contract.

5. **Publish `schemas/history.json` and register it as `history`.** Closed
   schema (`additionalProperties: false`), all six keys required, guarded by the
   existing schema-drift test against a maximally-populated `HistoryRecord`.

## Consequences

- The flattened record shape becomes a committed `--json` contract; renaming a
  key or changing `comment_id`'s semantics later is a breaking change tracked by
  the envelope `schema_version` and the drift test.
- `bug history --json` costs two API round-trips (history + comments) instead of
  one; table mode still costs one. The extra call is the price of `comment_id`.
- Correlation is timestamp-precision-bound: a comment and a field change in the
  same entry will match when the server stamps them with the same second, which
  is the normal case for "I changed status and left a comment in one submit". A
  server that records sub-second skew between the two would miss the link and
  emit null — acceptable degradation, never a wrong id.
- Agents get a directly diffable per-field stream; the `bzr-triage-bug`
  read-before-write rule becomes machine-checkable.

## Considered & rejected

- **Always emit `comment_id: null`.** Rejected: the issue explicitly wants the
  link "if the change carried a comment", and the correlation, while
  timestamp-bound, is reliable for the common single-submit case. A permanently
  null field would be dead weight in the published schema.
- **Drop `comment_id` from the shape entirely.** Rejected: deviates from the
  issue's documented record shape and removes the field even as a forward-compat
  placeholder; agents lose the comment linkage the feature is meant to provide.
- **Make the comment fetch fatal (propagate its error).** Rejected: the change
  delta is the core contract and is already in hand when the comment fetch runs;
  failing the whole command because comment listing was blocked would deny an
  agent the history it successfully retrieved. Best-effort-with-warning mirrors
  the graceful-degradation stance in [ADR 0006](0006-bug-links-isolated-fetch.md).
- **Keep the grouped shape and add the flat one under a new flag.** Rejected: no
  released schema pins the grouped JSON, so there is nothing to preserve; a dual
  shape doubles the contract surface and the test matrix for no consumer benefit
  (per the repo's "replace, don't deprecate" rule).
- **Carry `attachment_id` into the JSON record.** Rejected: the issue's
  documented shape is exactly six keys; attachment changes still surface as a
  `field`/`old_value`/`new_value` record, and the attachment number stays in the
  table. Revisit only if a consumer needs the structured attachment id.
- **Correlate by array position or comment count instead of timestamp.**
  Rejected: history entries and comments are independent lists with no shared
  index; only `(who, when)` is common to both, so the timestamp key is the only
  sound join.
