#!/usr/bin/env bash
set -euo pipefail

bug_id=$1
demo_root=$2
skill_root=$3
fields='["id","summary","status","resolution","assigned_to","priority","severity","target_milestone","deadline","last_change_time","whiteboard","blocks","depends_on"]'
rules='{"terminal_statuses":["RESOLVED","CLOSED"],"stale_after_days":10}'
snapshot_root="$demo_root/snapshots"
mkdir -p "$snapshot_root/.staging"

collect() {
  local created_at=$2 stage="$snapshot_root/.staging/$1"
  bzr query show core-weekly --json >"$demo_root/query.json"
  bzr query run core-weekly --fields "$(jq -r 'join(",")' <<<"$fields")" --paginate --json \
    >"$demo_root/bugs.json"
  mkdir "$stage"
  fingerprint=$("$skill_root/scripts/scope-fingerprint.sh" <"$demo_root/query.json")
  jq --arg created_at "$created_at" --arg server demo --arg scope_label core-weekly \
    --arg scope_fingerprint "$fingerprint" --argjson fields "$fields" --argjson rules "$rules" \
    -f "$skill_root/scripts/build-snapshot.jq" "$demo_root/bugs.json" >"$stage/snapshot.json"
  printf '%s\n' "$stage"
}

first_stage=$(collect baseline 2026-08-21T12:00:00Z)
printf '# Core Platform weekly status\n\nNo compatible prior snapshot exists; this report establishes the baseline.\n' \
  >"$first_stage/report.md"
"$skill_root/scripts/publish-run.sh" "$snapshot_root" baseline "$first_stage" >/dev/null
printf '%s\n' 'Baseline published.'

bzr bug update "$bug_id" --status IN_PROGRESS --whiteboard 'blocked: parser owner needed' >/dev/null
second_stage=$(collect comparison 2026-08-28T12:00:00Z)
previous=$("$skill_root/scripts/select-baseline.sh" "$second_stage/snapshot.json" \
  "$snapshot_root/runs" '["id","status","whiteboard"]')
jq -n --slurpfile previous "$previous" --slurpfile current "$second_stage/snapshot.json" \
  --argjson required_fields '["id","status","whiteboard"]' \
  -f "$skill_root/scripts/compare-snapshots.jq" >"$second_stage/comparison.json"
jq -r '"# Core Platform weekly status\n\n## Facts\n\n- Status transitions: \(.transitions | length)\n- Newly resolved: \(.newly_resolved | length)\n\n## Interpretation\n\n- Review changed blockers with the project owner.\n"' \
  "$second_stage/comparison.json" >"$second_stage/report.md"
"$skill_root/scripts/publish-run.sh" "$snapshot_root" comparison "$second_stage" >/dev/null
jq '{newly_resolved, transitions, stale_crossed, attention_unchanged}' \
  "$snapshot_root/latest/comparison.json"
