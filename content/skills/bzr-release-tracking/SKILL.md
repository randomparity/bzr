---
name: bzr-release-tracking
description: Use when driving a Bugzilla release milestone with bzr — "what's left for the 9.0 milestone?", checking which bugs drifted into or out of the release, rebalancing assignees and CC before the ship date, building a release-ready digest, or closing out the milestone once it ships.
---

# Track a release milestone with bzr

The workflow for one Target Milestone: enumerate what is open, detect drift,
rebalance ownership, publish a digest, then close the milestone out. No new
commands — this composes `bzr bug list`, `bzr bug history`, and `bzr bug
update`.

This reference is authored against **bzr 0.8.2-dev**.

## 1. Enumerate the milestone

```
# Everything not closed on the 9.0 milestone
bzr bug list --target-milestone 9.0 --status \!CLOSED --paginate --json \
  | jq -r '.data[] | "\(.id)\t\(.status)\t\(.assigned_to // "-")\t\(.summary)"'

# Headline number only
bzr bug list --target-milestone 9.0 --status \!CLOSED --count          # table: integer
bzr bug list --target-milestone 9.0 --status \!CLOSED --count --json | jq '.data.count'
```

Filter rules that matter here:

- `--status` (and every other filter) is **repeatable for OR** within the
  category: `--status CONFIRMED --status IN_PROGRESS` matches either. Prefix a
  value with `!` to invert, so `--status \!CLOSED` is "everything but CLOSED".
- `--target-milestone` follows the same rule: repeat it to watch two
  milestones at once (`--target-milestone 9.0 --target-milestone 9.1`), or
  negate it to find bugs parked *out* of the release.
- `--paginate` fetches every page; without it you get only the first page
  (default limit) and silently miss bugs past page one.
- **`--count` cannot combine with `--paginate` or `--offset`** — it is a
  server-side count of all matches. Use it for the headline number, use
  `--paginate` when you need the rows.

## 2. Detect drift

A bug has drifted if its `target_milestone` moved after your baseline (the
branch point, the triage freeze, last week's review). `bug history <id>
--json` gives the structured record — no screen-scraping:

```
# Every target_milestone change on one bug: when, who, old -> new
bzr bug history 12345 --json \
  | jq -r '.data[]
      | select(.field == "target_milestone")
      | "\(.when)\t\(.who)\t\(.old_value // "-") -> \(.new_value // "-")"'

# Only changes since the branch point
bzr bug history 12345 --since 2026-08-01 --json \
  | jq -r '.data[] | select(.field == "target_milestone") | "\(.who) moved it to \(.new_value)"'
```

Each record carries `when`, `who`, `field`, `old_value`, `new_value`, and
`comment_id` (best-effort correlation to the comment announcing the change;
`null` when there is none). To sweep the whole milestone for drift, stream the
bugs first (section 5's loop shape) and run the history filter per bug.

To see where a bug went instead of where it was, flip the negation:
`--target-milestone \!9.0 --changed-since <baseline>` lists bugs that changed
at all since the baseline while sitting outside the milestone.

## 3. Rebalance assignee and CC

Resolve a person's name to their login first — never guess an email. The
login is the account's `email` in the `user search` JSON:

```
bzr user search "Alice Example" --json | jq -r '.data[].email'
```

Then reassign and adjust CC. CC edits are incremental pairs — add and remove
in one call; there is **no bare `--cc` setter** on `bug update`:

```
# Preview first (--dry-run prints the payload, writes nothing), then commit
bzr bug update 12345 --assignee alice@example.com --dry-run
bzr bug update 12345 --assignee alice@example.com

# Swap watchers in the same write; comma-delimited on both flags
bzr bug update 12345 --cc-add alice@example.com,bob@example.com \
                     --cc-remove carol@example.com --dry-run
```

`--dry-run` is a global flag: it resolves and validates each mutation and
prints `"action":"dry-run"` with exit 0, so run it once per intended write
before the real one. For a whole-milestone rebalance, drive both passes from
one loop — see `bzr-bulk-triage` for the loop, read-before-write rule, and
concurrency bounds rather than re-deriving them here.

## 4. Release-ready digest

Project exactly the columns stakeholders read and emit a markdown table:

```
bzr bug list --target-milestone 9.0 --status \!CLOSED \
  --fields id,summary,status,assignee,priority --paginate --json \
  | jq -r '.data | sort_by(.priority)[]
      | "| #\(.id) | \(.summary) | \(.status) | \(.assigned_to // "-") | \(.priority // "-") |"'
```

Wrap it with the header rows:

```markdown
| Bug | Summary | Status | Assignee | Priority |
|-----|---------|--------|----------|----------|
| #101 | Crash on startup | NEW | alice@example.com | P1 |
| #102 | Slow search | ASSIGNED | bob@example.com | P2 |
```

More extraction shapes — sorted bullets, ndjson streaming for large sets —
are in `bzr-search-report` under "Build a digest"; reuse those patterns here
instead of inventing variants.

## 5. Close out the milestone

Once the release ships, bulk-transition what remains. Negations OR together,
so exclude every terminal state you do not want to touch:
```
bzr bug list --target-milestone 9.0 --status \!RESOLVED --status \!VERIFIED --status \!CLOSED \
  --paginate --output ndjson \
  | while IFS= read -r line; do
      id=$(printf '%s' "$line" | jq -r '.id')
      bzr bug update "$id" --status RESOLVED --resolution FIXED --dry-run &&
        bzr bug update "$id" --status RESOLVED --resolution FIXED
    done
```

Do **not** hand-roll the batch machinery: `bzr-bulk-triage` owns the
stream-with-ndjson pattern, per-row read-before-write, the
`--expect-unchanged-since` collision guard, `--dry-run` → real-write
discipline, and concurrency caps. Follow its dry-run-then-write loop verbatim
and swap in the `--status RESOLVED --resolution FIXED` transition above.
Before closing anything, confirm the allowed transitions with
`bzr field list status --json` (`can_change_to`) — servers differ on whether
NEW can jump straight to RESOLVED.
