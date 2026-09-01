#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
FILTER="$HERE/../scripts/compare-snapshots.jq"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

snapshot() {
	jq -n --arg server "$1" --arg fingerprint "$2" --argjson bugs "$3" '{format_version:1,created_at:"2026-08-28T00:00:00Z",server:$server,scope_label:"core-weekly",scope_fingerprint:$fingerprint,fields:["assigned_to","blocks","deadline","depends_on","id","last_change_time","priority","resolution","severity","status","summary","target_milestone","whiteboard"],rules:{terminal_statuses:["RESOLVED","CLOSED"],stale_after_days:10},bugs:$bugs,limitations:[]}'
}

fp=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
snapshot server-a "$fp" '{"1":{"id":1,"summary":"old","status":"NEW","resolution":""}}' >"$WORK/old.json"
snapshot server-a "$fp" '{"1":{"id":1,"summary":"new","status":"RESOLVED","resolution":"FIXED"},"2":{"id":2,"summary":"=formula","status":"NEW","resolution":""}}' >"$WORK/new.json"

: >"$WORK/no-previous.json"
jq -n --slurpfile previous "$WORK/no-previous.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["id","summary","status","resolution"]' -f "$FILTER" >"$WORK/baseline.json"
[ "$(jq -r '.baseline' "$WORK/baseline.json")" = true ]

jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["id","summary","status","resolution"]' -f "$FILTER" >"$WORK/changed.json"
[ "$(jq -r '.added[0]' "$WORK/changed.json")" = 2 ]
[ "$(jq -r '.newly_resolved[0]' "$WORK/changed.json")" = 1 ]
[ "$(jq -r '.transitions[] | select(.field=="status") | .id' "$WORK/changed.json")" = 1 ]

snapshot server-a "$fp" '{"2":{"id":2,"summary":"still here","status":"NEW","resolution":""}}' >"$WORK/removed.json"
jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/removed.json" --argjson required_fields '["id"]' -f "$FILTER" >"$WORK/removal.json"
[ "$(jq -r '.removed_from_scope[0]' "$WORK/removal.json")" = 1 ]
[ "$(jq -r 'has("closed")' "$WORK/removal.json")" = false ]

jq '.limitations=[{"id":3,"reason":"inaccessible"}]' "$WORK/new.json" >"$WORK/limited.json"
jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/limited.json" --argjson required_fields '["id"]' -f "$FILTER" >"$WORK/limited-result.json"
[ "$(jq -r '.limitations[0].reason' "$WORK/limited-result.json")" = inaccessible ]

if jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["custom_missing"]' -f "$FILTER" >/dev/null 2>&1; then
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

jq '.rules.stale_after_days=14' "$WORK/new.json" >"$WORK/wrong-rules.json"
if jq -n --slurpfile previous "$WORK/old.json" --slurpfile current "$WORK/wrong-rules.json" --argjson required_fields '["id"]' -f "$FILTER" >/dev/null 2>&1; then
	echo 'expected incompatible rules failure' >&2
	exit 1
fi

jq '.rules.terminal_statuses=["CLOSED","RESOLVED","CLOSED"]' "$WORK/old.json" >"$WORK/reordered-rules.json"
jq -n --slurpfile previous "$WORK/reordered-rules.json" --slurpfile current "$WORK/new.json" --argjson required_fields '["id"]' -f "$FILTER" >"$WORK/reordered-result.json"
[ "$(jq -r '.baseline' "$WORK/reordered-result.json")" = false ]

root="$WORK/published"
mkdir -p "$root/.staging"
stage="$root/.staging/stage-ok"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
printf '# report\n' >"$stage/report.md"
"$HERE/../scripts/publish-run.sh" "$root" run-1 "$stage" >/dev/null
[ "$(readlink "$root/latest")" = runs/run-1 ]
[ -f "$root/runs/run-1/report.md" ]

stage="$root/.staging/stage-second"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
"$HERE/../scripts/publish-run.sh" "$root" run-2 "$stage" >/dev/null
[ "$(readlink "$root/latest")" = runs/run-2 ]
[ -f "$root/runs/run-1/snapshot.json" ]
[ -f "$root/runs/run-2/snapshot.json" ]

mkdir "$WORK/race-bin"
printf '%s\n' '#!/bin/sh' \
  'case "$3" in' \
  '*/latest) case "$2" in */.latest.*) rm "$3"; mkdir "$3";; esac;;' \
  'esac' \
  'exec /bin/mv "$@"' >"$WORK/race-bin/mv"
chmod +x "$WORK/race-bin/mv"
stage="$root/.staging/stage-race"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
if PATH="$WORK/race-bin:$PATH" "$HERE/../scripts/publish-run.sh" "$root" run-race "$stage" \
  >"$WORK/race.out" 2>"$WORK/race.err"; then
	echo 'expected pointer replacement race failure' >&2
	exit 1
fi
[ -d "$root/latest" ]
grep -q "new run retained: $root/runs/run-race" "$WORK/race.err"
[ -f "$root/runs/run-race/snapshot.json" ]
if find "$root/latest" -name '.latest.run-race.*' -print -quit | grep -q .; then
	echo 'expected temporary pointer cleanup after replacement race' >&2
	exit 1
fi

stage="$root/.staging/stage-fail"
mkdir "$stage"
cp "$WORK/new.json" "$stage/snapshot.json"
rm "$root/latest"
mkdir "$root/latest"
if "$HERE/../scripts/publish-run.sh" "$root" run-3 "$stage" \
  >"$WORK/pointer-failure.out" 2>"$WORK/pointer-failure.err"; then
	echo 'expected pointer replacement failure' >&2
	exit 1
fi
[ -d "$root/latest" ]
grep -q 'latest is a directory' "$WORK/pointer-failure.err"
grep -q "new run retained: $root/runs/run-3" "$WORK/pointer-failure.err"
[ -f "$root/runs/run-1/snapshot.json" ]
[ -f "$root/runs/run-2/snapshot.json" ]
[ -f "$root/runs/run-3/snapshot.json" ]

invalid_stage="$root/.staging/invalid"
mkdir "$invalid_stage"
jq '.unexpected=true' "$WORK/new.json" >"$invalid_stage/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" invalid "$invalid_stage" >/dev/null 2>&1; then
	echo 'expected schema allowlist rejection' >&2
	exit 1
fi
[ ! -e "$root/runs/invalid" ]

sensitive_stage="$root/.staging/sensitive"
mkdir "$sensitive_stage"
jq '.rules.token="secret"' "$WORK/new.json" >"$sensitive_stage/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" sensitive "$sensitive_stage" >/dev/null 2>&1; then
	echo 'expected sensitive key rejection' >&2
	exit 1
fi

typed_stage="$root/.staging/typed"
mkdir "$typed_stage"
jq '.created_at="not-a-date" | .bzr_schema_version=7 | .rules.terminal_statuses=[7] | .rules.stale_after_days=1.5 | .limitations=[{"id":"wrong","reason":"bad"}]' "$WORK/new.json" >"$typed_stage/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" typed "$typed_stage" >/dev/null 2>&1; then
	echo 'expected schema type rejection' >&2
	exit 1
fi
[ ! -e "$root/runs/typed" ]

fractional_stage="$root/.staging/fractional"
mkdir "$fractional_stage"
jq '.created_at="2026-08-28T00:00:00.123Z"' "$WORK/new.json" >"$fractional_stage/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" fractional "$fractional_stage" >"$WORK/fractional.out" 2>"$WORK/fractional.err"; then
	echo 'expected fractional timestamp rejection' >&2
	exit 1
fi
grep -q 'snapshot-v1.schema.json' "$WORK/fractional.err"

symlink_stage="$root/.staging/symlink"
mkdir "$symlink_stage"
cp "$WORK/new.json" "$WORK/external-snapshot.json"
ln -s "$WORK/external-snapshot.json" "$symlink_stage/snapshot.json"
if "$HERE/../scripts/publish-run.sh" "$root" symlink "$symlink_stage" >/dev/null 2>&1; then
	echo 'expected staged symlink rejection' >&2
	exit 1
fi
[ ! -e "$root/runs/symlink" ]

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
tuple_a=$(printf '%s' '{"raw_params":[["f1","component"]]}' | "$HERE/../scripts/scope-fingerprint.sh")
tuple_b=$(printf '%s' '{"raw_params":[["component","f1"]]}' | "$HERE/../scripts/scope-fingerprint.sh")
[ "$tuple_a" != "$tuple_b" ]

runs="$WORK/history/runs"
mkdir -p "$runs/run-a" "$runs/run-b" "$runs/run-c"
jq '.created_at="2026-08-01T00:00:00Z"' "$WORK/old.json" >"$runs/run-a/snapshot.json"
jq '.created_at="2026-08-02T00:00:00Z" | .scope_fingerprint="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"' "$WORK/old.json" >"$runs/run-b/snapshot.json"
jq '.created_at="2026-08-03T00:00:00Z"' "$WORK/new.json" >"$runs/run-c/snapshot.json"
selected=$("$HERE/../scripts/select-baseline.sh" "$runs/run-c/snapshot.json" "$runs" '["id","status"]')
[ "$selected" = "$runs/run-a/snapshot.json" ]

jq '.created_at="2026-08-01T00:00:00Z" | .rules.terminal_statuses=["CLOSED","RESOLVED","CLOSED"]' "$WORK/old.json" >"$runs/run-a/snapshot.json"
selected=$("$HERE/../scripts/select-baseline.sh" "$runs/run-c/snapshot.json" "$runs" '["id","status"]')
[ "$selected" = "$runs/run-a/snapshot.json" ]

jq '.created_at="2026-08-01T00:00:00Z" | .bugs={"9":{"id":9,"summary":"idle","status":"NEW","resolution":"","last_change_time":"2026-07-25T00:00:00Z"}}' "$WORK/old.json" >"$WORK/stale-old.json"
jq '.created_at="2026-08-08T00:00:00Z" | .bugs={"9":{"id":9,"summary":"idle","status":"NEW","resolution":"","last_change_time":"2026-07-25T00:00:00Z"}}' "$WORK/new.json" >"$WORK/stale-new.json"
jq -n --slurpfile previous "$WORK/stale-old.json" --slurpfile current "$WORK/stale-new.json" --argjson required_fields '["id","last_change_time"]' -f "$FILTER" >"$WORK/stale-result.json"
[ "$(jq -r '.stale_crossed[0]' "$WORK/stale-result.json")" = 9 ]
[ "$(jq -r '.attention_unchanged[0]' "$WORK/stale-result.json")" = 9 ]

[ "$(printf '%s' '=cmd' | jq -Rr -L "$HERE/../scripts" 'include "safe-output"; spreadsheet_text')" = "'=cmd" ]
printf '\t=cmd' | jq -Re -L "$HERE/../scripts" 'include "safe-output"; spreadsheet_text | startswith("'\''")' >/dev/null
printf '\r=cmd' | jq -Re -L "$HERE/../scripts" 'include "safe-output"; spreadsheet_text | startswith("'\''")' >/dev/null
printf '\n=cmd' | jq -Rse -L "$HERE/../scripts" 'include "safe-output"; spreadsheet_text | startswith("'\''")' >/dev/null
[ "$(printf '%s' '<b>&' | jq -Rr -L "$HERE/../scripts" 'include "safe-output"; html_text')" = '&lt;b&gt;&amp;' ]

grep -q 'WS-10' "$HERE/../reference/eval-cases.md"
if grep -Eq 'bzr (bug|comment|attachment) (create|update|close|resolve|reopen|dup|add|upload|delete)' "$HERE/../SKILL.md"; then
	echo 'skill documents a forbidden mutation command' >&2
	exit 1
fi

printf '%s\n' 'weekly-status fixtures: ok'
