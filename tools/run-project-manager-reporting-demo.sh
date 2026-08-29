#!/usr/bin/env bash
set -euo pipefail

output=$1
: "${BZR_BIN:?}"

collection=$("$BZR_BIN" --server demo --json query run pm-demo \
  --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
  --paginate)
jq -r '
  .data as $bugs |
  ($bugs | map(.target_milestone // "Unscheduled") | group_by(.) |
    map({milestone: .[0], count: length})) as $milestones |
  "# Platform portfolio — current Bugzilla snapshot\n\n" +
  "## Executive summary\n\n" +
  "- \($bugs | length) open item(s) are in the saved-query scope.\n" +
  "- \($bugs | map(select((.whiteboard // "") | test("blocked|risk"; "i"))) | length) item(s) carry a current blocker or risk update.\n\n" +
  "## Milestone view\n\n" +
  ($milestones | map("- **\(.milestone):** \(.count) item(s)") | join("\n")) +
  "\n\n## Needs attention\n\n" +
  ($bugs | map(select((.whiteboard // "") | test("blocked|risk"; "i")) |
    "- **Bug \(.id) — \(.summary):** `\(.whiteboard)`") |
    if length == 0 then "- None observed." else join("\n") end) +
  "\n\n## Current updates\n\n" +
  ($bugs | map("- **Bug \(.id):** `\(.whiteboard // "No whiteboard update")`") | join("\n")) +
  "\n\n## Limitations\n\n- Whiteboard values are mutable snapshots; comments hold durable history.\n" +
  "\n## Provenance\n\n- Saved query `pm-demo`; projected fields; complete paginated JSON collection.\n"
' <<<"$collection" >"$output"
