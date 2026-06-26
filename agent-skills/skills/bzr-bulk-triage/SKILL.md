---
name: bzr-bulk-triage
description: Use when triaging or mutating many Bugzilla bugs from one query with bzr — "triage my open bugs", "close out the 5.x milestone", a weekly sweep. Stream the result set with --paginate --output ndjson, apply the read-before-write rule per row, and guard every mutation with --dry-run preview before the real write.
---

# Bulk-triage bugs with bzr (stream, preview, then write)

Single-bug triage is `bzr-triage-bug`. This skill scales that rule to a **query
result**: walk every matching bug, decide per bug, and mutate safely — without
loading the whole set into memory or firing blind writes.

The cardinal rule still holds, once per row: **read the current state before you
change anything.** A blind `bzr bug update` can overwrite a field someone else
just set. Batching does not relax this; it multiplies the blast radius.

## 1. Stream the result set (`--paginate --output ndjson`)

For a batch, prefer streaming over `--json`:

```
bzr bug list --product Foo --status NEW --paginate --output ndjson \
  | while IFS= read -r line; do
      id=$(printf '%s' "$line" | jq -r '.id')
      # ... act on $id ...
    done
```

Why this beats `--json` for large sets:

- **`--paginate`** fetches every page; a bare `--json` returns only the first
  page (default limit), so a one-shot loop silently skips bugs past page one.
- **`--output ndjson`** emits one compact record per line, so you act on each
  bug as it arrives instead of buffering the whole array. ndjson records are
  **bare** — no `schema_version` envelope and no `.data` wrapper — so read fields
  directly (`.id`, `.status`), not `.data[].id`. (`--paginate` and `--offset`
  are mutually exclusive; pick paging or a single window, not both.)

See `bzr-search-report` for the full filter/sort/count surface of `bug list` and
`query run`, and `bzr-reference` (`reference/json-recipes.md`) for the ndjson
contract.

## 2. Read before write, per row

Inside the loop, fetch the full current state before deciding — the
`bzr-triage-bug` rule applied to each bug:

```
bzr bug list --product Foo --status NEW --paginate --output ndjson \
  | while IFS= read -r line; do
      id=$(printf '%s' "$line" | jq -r '.id')
      # read-before-write: pull live state, not the streamed snapshot
      bzr bug view "$id" --json \
        | jq -r '.data | "#\(.id) [\(.status)] \(.assigned_to) — \(.summary)"'
    done
```

Use `bug view --json` (single object at `.data`) for the decision, not the
streamed ndjson row: the row is a list-view snapshot and may omit fields, and a
slow loop can race a concurrent edit. `bzr bug history <id>` and
`bzr comment list <id>` add who-changed-what context when a bug needs judgement.

## 3. Preview, verify, then write (`--dry-run` → real write)

Treat each mutation as an agent tool call with a preview. `--dry-run` validates
and prints the would-be payload (`"action":"dry-run"`) **without** calling the
API; drop the flag to commit:

```
      # 1. preview — no write
      bzr bug update "$id" --status ASSIGNED --dry-run
      # 2. inspect the payload, then commit the same command without --dry-run
      bzr bug update "$id" --status ASSIGNED
```

Make the write collision-safe with the same `--expect-unchanged-since` guard
`bzr-triage-bug` uses — capture `last_change_time` at read time and pass it back
so a concurrent edit is rejected (exit 14) instead of clobbered:

```
      ts=$(bzr bug view "$id" --json | jq -r '.data.last_change_time')
      bzr bug update "$id" --status ASSIGNED --expect-unchanged-since "$ts"
```

If a status transition is rejected, `bzr field list status --json` lists the
allowed `can_change_to` targets from the current state.

### `--yes` vs. `--dry-run`

These are different guards — do not confuse them:

- **`--dry-run`** previews one command without writing. Use it to verify intent.
- **`-y` / `--yes`** skips the *interactive batch-confirmation prompt* that fires
  when a **single** `bzr bug update` touches more than 10 bugs at a TTY. It does
  not preview anything.

A per-row loop calls `bug update` once per bug (one bug each), so it never trips
that prompt — the loop's safety comes from `--dry-run` and
`--expect-unchanged-since`, not `--yes`. `--yes` is for the one-shot batch form:

```
# One command, many bugs — prompts above 10 at a TTY; --yes skips the prompt
bzr bug update 101 102 103 104 --status RESOLVED --resolution WONTFIX --dry-run
bzr bug update 101 102 103 104 --status RESOLVED --resolution WONTFIX --yes
```

## 4. Bound your concurrency

Do not background a write per row (`bzr bug update … &`) across a whole query —
that floods the server with unbounded parallel `Bug.update` calls and interleaves
output unreadably. A serial loop is the safe default. If you must parallelize,
cap it. With GNU `xargs`:

```
bzr bug list --product Foo --status NEW --paginate --output ndjson \
  | jq -r '.id' \
  | xargs -P 4 -I{} bzr bug update {} --status ASSIGNED --expect-unchanged-since "$ts"
```

`-P 4` is a deliberate ceiling. Prefer serial unless the set is large and the
mutation is independent per bug; never let the fan-out grow with the result size.

## 5. `bug my` vs. `bug list`

When the batch is *your own* work, prefer `bug my` — it derives the identity
filter from the authenticated user, so you cannot typo your own email:

```
# identity-derived: "my open bugs", no --assignee needed
bzr bug my --status \!CLOSED --product Foo --paginate --output ndjson

# explicit filter: someone else's, or a cross-cutting product/milestone sweep
bzr bug list --assignee alice@example.com --status NEW --paginate --output ndjson
```

Use `bug my` for self-triage; use `bug list` for a filtered or milestone-wide
sweep that is not tied to the caller. Both stream identically. `bug my` also
takes `--cc`, `--created`, and `--all` to widen the identity net (see
`bzr-search-report`).

## 6. Saved queries as the entry point

Save the triage query once, then drive the sweep by name so the filter is
reviewed and reused instead of retyped each run:

```
bzr query save triage-foo --product Foo --status NEW --status ASSIGNED
bzr query run triage-foo --paginate --output ndjson \
  | while IFS= read -r line; do
      id=$(printf '%s' "$line" | jq -r '.id')
      bzr bug view "$id" --json | jq -r '.data | "#\(.id) \(.status)"'
      bzr bug update "$id" --status IN_PROGRESS --dry-run
    done
```

See `bzr-search-report` for saving, updating (`query update --from-url`), and
listing queries, and `bzr-triage-bug` for the per-bug read/decide/verify detail
this skill repeats across a set.
