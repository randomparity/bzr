#!/bin/sh
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
FILTER="$HERE/../scripts/compare-snapshots.jq"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

snapshot() {
  jq -n --arg server "$1" --arg fingerprint "$2" --argjson bugs "$3" '{format_version:1,created_at:"2026-08-28T00:00:00Z",server:$server,scope_label:"core-weekly",scope_fingerprint:$fingerprint,fields:["id","resolution","status","summary"],rules:{},bugs:$bugs,limitations:[]}'
}

fp=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
snapshot server-a "$fp" '{"1":{"id":1,"summary":"old","status":"NEW","resolution":""}}' >"$WORK/old.json"
snapshot server-a "$fp" '{"1":{"id":1,"summary":"new","status":"RESOLVED","resolution":"FIXED"},"2":{"id":2,"summary":"=formula","status":"NEW","resolution":""}}' >"$WORK/new.json"

: >"$WORK/no-previous.json"
jq -n --slurpfile previous "$WORK/no-previous.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["id","summary","status","resolution"]' -f "$FILTER" >"$WORK/baseline.json"
[ "$(jq -r '.baseline' "$WORK/baseline.json")" = true ]

jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["id","summary","status","resolution"]' -f "$FILTER" >"$WORK/changed.json"
[ "$(jq -r '.added[0]' "$WORK/changed.json")" = 2 ]
[ "$(jq -r '.changed[0].id' "$WORK/changed.json")" = 1 ]

snapshot server-a "$fp" '{"2":{"id":2,"summary":"still here","status":"NEW","resolution":""}}' >"$WORK/removed.json"
jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/removed.json" --argjson required_fields '["id"]' -f "$FILTER" >"$WORK/removal.json"
[ "$(jq -r '.removed_from_scope[0]' "$WORK/removal.json")" = 1 ]
[ "$(jq -r 'has("closed")' "$WORK/removal.json")" = false ]

jq '.limitations=[{"id":3,"reason":"inaccessible"}]' "$WORK/new.json" >"$WORK/limited.json"
jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/limited.json" --argjson required_fields '["id"]' -f "$FILTER" >"$WORK/limited-result.json"
[ "$(jq -r '.limitations[0].reason' "$WORK/limited-result.json")" = inaccessible ]

if jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["deadline"]' -f "$FILTER" >/dev/null 2>&1; then
  echo 'expected incompatible fields failure' >&2
  exit 1
fi

jq '.server="server-b"' "$WORK/new.json" >"$WORK/wrong-server.json"
if jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/wrong-server.json" --argjson required_fields '["id"]' -f "$FILTER" >/dev/null 2>&1; then
  echo 'expected incompatible server failure' >&2
  exit 1
fi

jq '.scope_fingerprint="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"' "$WORK/new.json" >"$WORK/wrong-scope.json"
if jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/wrong-scope.json" --argjson required_fields '["id"]' -f "$FILTER" >/dev/null 2>&1; then
  echo 'expected changed effective query failure' >&2
  exit 1
fi

root="$WORK/published"
mkdir -p "$root/.staging"
stage="$root/.staging/stage-ok"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
printf '# report\n' >"$stage/report.md"
"$HERE/../scripts/publish-run.sh" "$root" run-1 "$stage" >/dev/null
[ "$(readlink "$root/latest")" = runs/run-1 ]
[ -f "$root/runs/run-1/report.md" ]

stage="$root/.staging/stage-fail"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
rm "$root/latest"
mkdir "$root/latest"
if "$HERE/../scripts/publish-run.sh" "$root" run-2 "$stage" >/dev/null 2>&1; then
  echo 'expected pointer replacement failure' >&2
  exit 1
fi
[ -f "$root/runs/run-1/snapshot.json" ]
[ -f "$root/runs/run-2/snapshot.json" ]

outside="$WORK/outside-stage"
mkdir "$outside"
cp "$WORK/new.json" "$outside/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" run-3 "$outside" >/dev/null 2>&1; then
  echo 'expected outside staging rejection' >&2
  exit 1
fi

query_a='{"name":"core-weekly","product":["B","A"],"status":["NEW"],"source_url":"https://u:p@example.invalid/?api_key=secret","updated_at":"one"}'
query_b='{"updated_at":"two","source_url":"https://example.invalid/other","status":["NEW"],"product":["A","B"],"name":"renamed"}'
fp_a=$(printf '%s' "$query_a" | "$HERE/../scripts/scope-fingerprint.sh")
fp_b=$(printf '%s' "$query_b" | "$HERE/../scripts/scope-fingerprint.sh")
[ "$fp_a" = "$fp_b" ]
fp_c=$(printf '%s' '{"name":"core-weekly","product":["C"],"status":["NEW"]}' | "$HERE/../scripts/scope-fingerprint.sh")
[ "$fp_a" != "$fp_c" ]

[ "$(printf '%s' '=cmd' | jq -Rr -L "$HERE/../scripts" 'include "safe-output"; spreadsheet_text')" = "'=cmd" ]
[ "$(printf '%s' '<b>&' | jq -Rr -L "$HERE/../scripts" 'include "safe-output"; html_text')" = '&lt;b&gt;&amp;' ]
[ "$(printf '%s' 'javascript:alert(1)' | jq -Rr -L "$HERE/../scripts" 'include "safe-output"; safe_http_url')" = null ]

grep -q 'WS-10' "$HERE/../reference/eval-cases.md"
! grep -Eq 'bzr (bug|comment|attachment) (create|update|close|resolve|reopen|dup|add|upload|delete)' "$HERE/../SKILL.md"

printf '%s\n' 'weekly-status fixtures: ok'
