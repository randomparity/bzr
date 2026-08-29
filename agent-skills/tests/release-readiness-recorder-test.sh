#!/usr/bin/env bash
set -euo pipefail

test_root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd "$test_root/../.." && pwd -P)
work=$(mktemp -d)
trap 'rm -r "$work"' EXIT
real_mv=$(command -v mv)

sandbox="$work/repo"
stub_bin="$work/bin"
mkdir -p "$sandbox/tools" "$sandbox/docs/assets" "$stub_bin"
cp -p "$repo_root/tools/record-demo.sh" "$sandbox/tools/record-demo.sh"
cp -p "$repo_root/tools/run-release-readiness-demo.sh" \
  "$sandbox/tools/run-release-readiness-demo.sh"

tracked_cast="$sandbox/docs/assets/bzr-release-readiness-demo.cast"
tracked_gif="$sandbox/docs/assets/bzr-release-readiness-demo.gif"
printf 'published-cast\n' >"$tracked_cast"
printf 'published-gif\n' >"$tracked_gif"
cp "$tracked_cast" "$work/cast.before"
cp "$tracked_gif" "$work/gif.before"

cat >"$stub_bin/bzr" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case " $* " in
*' bug list '*' --whiteboard '*)
  printf '%s\n' '{"data":[{"id":1,"product":"ReleaseDemo","whiteboard":"bzr-release-readiness-demo-v1 dependency"},{"id":2,"product":"ReleaseDemo","whiteboard":"bzr-release-readiness-demo-v1 complete"},{"id":3,"product":"ReleaseDemo","whiteboard":"bzr-release-readiness-demo-v1 release-blocker"}]}'
  ;;
*' bug view '*)
  printf '%s\n' '{"data":{"id":3,"product":"ReleaseDemo","version":"9.0","target_milestone":"9.0","summary":"release root","status":"NEW","priority":"Highest","severity":"major","assigned_to":null,"deadline":"2030-08-31","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 release-blocker","depends_on":[1]}}'
  ;;
*' query show release-readiness-demo-url '*)
  printf '%s\n' '{"data":{"source_url":"https://bugzilla.example.invalid/buglist.cgi?product=ReleaseDemo"}}'
  ;;
*' query show '*)
  printf '%s\n' '{"data":{"name":"release-readiness-demo"}}'
  ;;
*' bug history '*)
  printf '%s\n' '{"data":[{"field":"status","removed":"RESOLVED","added":"REOPENED","when":"2030-08-01T00:00:00Z"}]}'
  ;;
*' bug links '*)
  printf '%s\n' '{"data":[{"id":1,"relation":"depends_on","status":"NEW"}]}'
  ;;
*' field list '*)
  printf '%s\n' '{"data":[]}'
  ;;
*' server capabilities '*)
  printf '%s\n' '{"data":{"custom_fields":[]}}'
  ;;
*' schema '*)
  printf '%s\n' '{"type":"object"}'
  ;;
*' bug search '* | *' query run '* | *' bug list '*)
  printf '%s\n' '{"data":[{"id":1,"product":"ReleaseDemo","version":"9.0","target_milestone":"9.0","summary":"dependency","status":"NEW","priority":"Normal","severity":"major","assigned_to":"owner@example.invalid","deadline":"2030-08-31","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 dependency","depends_on":[]},{"id":2,"product":"ReleaseDemo","version":"9.0","target_milestone":"9.0","summary":"complete","status":"RESOLVED","priority":"Highest","severity":"major","assigned_to":null,"deadline":"2030-07-01","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[]},{"id":3,"product":"ReleaseDemo","version":"9.0","target_milestone":"9.0","summary":"release root","status":"NEW","priority":"Highest","severity":"major","assigned_to":null,"deadline":"2030-08-31","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 release-blocker","depends_on":[1]}]}'
  ;;
esac
EOF
cat >"$stub_bin/date" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count=0
[[ ! -e $FAKE_DATE_STATE ]] || count=$(<"$FAKE_DATE_STATE")
case $count in
0) now=2030-08-30T00:00:00Z ;;
1) now=2030-08-30T00:00:05Z ;;
*) now=2030-08-30T00:00:10Z ;;
esac
printf '%s\n' "$((count + 1))" >"$FAKE_DATE_STATE"
printf '%s\n' "$now"
EOF
cat >"$stub_bin/curl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat >"$stub_bin/asciinema" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
for output; do :; done
printf '%s\n' "$FAKE_CAST_CONTENT" >"$output"
EOF
cat >"$stub_bin/agg" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
args=("$@")
input=${args[${#args[@]}-2]}
output=${args[${#args[@]}-1]}
printf '%s\n%s\n' "$input" "$output" >"$AGG_CALLED"
printf 'rendered-gif\n' >"$output"
[[ ${FAKE_AGG_FAIL:-0} -eq 0 ]]
EOF
cat >"$stub_bin/mv" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
args=("$@")
destination=${args[${#args[@]}-1]}
target=${FAKE_MV_FAIL_TARGET:-}
if [[ -n $target && ! -e ${FAKE_MV_STATE:?} &&
  $destination == *"bzr-release-readiness-demo.$target" ]]; then
  : >"$FAKE_MV_STATE"
  exit 1
fi
exec "$MV_REAL" "$@"
EOF
chmod +x "$stub_bin/bzr" "$stub_bin/date" "$stub_bin/curl" \
  "$stub_bin/asciinema" "$stub_bin/agg" "$stub_bin/mv"

common_env=(
  PATH="$stub_bin:$PATH"
  BZR_BIN="$stub_bin/bzr"
  AGG_CALLED="$work/agg-called"
  MV_REAL="$real_mv"
  FAKE_MV_STATE="$work/mv-failed"
)

rm -f "$work/date-state"
env "${common_env[@]}" FAKE_DATE_STATE="$work/date-state" \
  bash "$sandbox/tools/run-release-readiness-demo.sh" demo \
  bzr-release-readiness-demo-v1 "$work/timed-report.md" "$work/timed-trace.jsonl"
grep -Fq 'Generated: 2030-08-30T00:00:00Z' "$work/timed-report.md"
grep -Fq 'collection started 2030-08-30T00:00:05Z and ended 2030-08-30T00:00:10Z' \
  "$work/timed-report.md"
grep -Fq 'changed before 2030-07-31T00:00:00Z' "$work/timed-report.md"
grep -Fq '**Fact:** Ownership check: N/A (not selected).' "$work/timed-report.md"
grep -Fq '**Fact:** History/regression check: N/A (not selected); no history read was issued.' \
  "$work/timed-report.md"

jq -e '
  all(.[]; type == "array" and .[0:4] ==
    ["bzr", "--server", "<server-profile>", "--json"]) and
  any(.[]; .[4:6] == ["bug", "list"] and
    any(.[]; . == "bzr-release-readiness-demo-v1")) and
  ([.[] | select(.[4:6] == ["query", "show"])] | length) == 2 and
  any(.[]; .[4:7] == ["bug", "list", "--product"] and
    (index("id,summary,status,priority,severity,keywords,flags,depends_on,last_change_time,whiteboard") != null)) and
  all(.[]; .[4:6] != ["bug", "history"])
' < <(jq -s . "$work/timed-trace.jsonl") >/dev/null
jq -r 'join(" ")' "$work/timed-trace.jsonl" >"$work/trace-commands"
# shellcheck disable=SC2016 # The sed addresses are literal Markdown fences.
sed -n '/^```text$/,/^```$/p' "$work/timed-report.md" |
  sed '1d;$d' >"$work/report-commands"
cmp "$work/trace-commands" "$work/report-commands"

cat >"$work/fake-release-helper" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '# test report\n' >"$3"
EOF
chmod +x "$work/fake-release-helper"
env RELEASE_READINESS_DEMO_HELPER="$work/fake-release-helper" \
  RELEASE_READINESS_DEMO_MARKER=bzr-release-readiness-demo-v1 \
  RELEASE_READINESS_DEMO_REPORT="$work/driver-report.md" \
  RELEASE_READINESS_DEMO_SERVER=demo \
  RELEASE_READINESS_DEMO_PRODUCT=ReleaseDemo \
  BZR_BIN="$stub_bin/bzr" \
  bash "$sandbox/tools/record-demo.sh" --drive-release-readiness \
  >"$work/driver.stdout"
# shellcheck disable=SC2016 # Backticks are literal scope delimiters.
grep -Fq 'Use product `ReleaseDemo` as the only release scope.' "$work/driver.stdout"
grep -Fq 'Do not run deadline, ownership, milestone, status/resolution, or history/regression checks.' \
  "$work/driver.stdout"
grep -Fq 'Use a maximum of 100 root bugs and return a PM-ready Markdown report.' \
  "$work/driver.stdout"

if env "${common_env[@]}" FAKE_CAST_CONTENT='http://127.0.0.1:8089' \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/leak.stdout" 2>"$work/leak.stderr"; then
  printf 'recorder privacy failure: leaking cast was accepted\n' >&2
  exit 1
fi
cmp "$work/cast.before" "$tracked_cast" || {
  printf 'recorder privacy failure: unverified cast replaced the published cast\n' >&2
  exit 1
}
cmp "$work/gif.before" "$tracked_gif" || {
  printf 'recorder privacy failure: rejected cast replaced the published GIF\n' >&2
  exit 1
}
[[ ! -e $work/agg-called ]] || {
  printf 'recorder privacy failure: rejected cast reached the renderer\n' >&2
  exit 1
}

if env "${common_env[@]}" FAKE_CAST_CONTENT='verified-cast' FAKE_AGG_FAIL=1 \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/render-failure.stdout" 2>"$work/render-failure.stderr"; then
  printf 'recorder publication failure: failing renderer was accepted\n' >&2
  exit 1
fi
cmp "$work/cast.before" "$tracked_cast" || {
  printf 'recorder publication failure: failed render replaced the published cast\n' >&2
  exit 1
}
cmp "$work/gif.before" "$tracked_gif" || {
  printf 'recorder publication failure: failed render replaced the published GIF\n' >&2
  exit 1
}

for failed_asset in cast gif; do
  cp "$work/cast.before" "$tracked_cast"
  cp "$work/gif.before" "$tracked_gif"
  rm -f "$work/mv-failed"
  if env "${common_env[@]}" FAKE_CAST_CONTENT='verified-cast' \
    FAKE_MV_FAIL_TARGET="$failed_asset" \
    bash "$sandbox/tools/record-demo.sh" release-readiness \
    >"$work/$failed_asset-failure.stdout" \
    2>"$work/$failed_asset-failure.stderr"; then
    printf 'recorder publication failure: failing %s move was accepted\n' \
      "$failed_asset" >&2
    exit 1
  fi
  cmp "$work/cast.before" "$tracked_cast" || {
    printf 'recorder publication failure: %s failure did not restore cast\n' \
      "$failed_asset" >&2
    exit 1
  }
  cmp "$work/gif.before" "$tracked_gif" || {
    printf 'recorder publication failure: %s failure did not restore GIF\n' \
      "$failed_asset" >&2
    exit 1
  }
done

rm -f "$work/mv-failed"
env "${common_env[@]}" FAKE_CAST_CONTENT='verified-cast' \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/clean.stdout" 2>"$work/clean.stderr"
grep -Fxq 'verified-cast' "$tracked_cast"
grep -Fxq 'rendered-gif' "$tracked_gif"
if grep -Fxq "$tracked_cast" "$work/agg-called" ||
  grep -Fxq "$tracked_gif" "$work/agg-called"; then
  printf 'recorder publication failure: renderer used a published asset path\n' >&2
  exit 1
fi

printf 'release-readiness recorder privacy: ok\n'
