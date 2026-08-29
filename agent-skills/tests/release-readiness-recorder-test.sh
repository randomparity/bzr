#!/usr/bin/env bash
# shellcheck disable=SC2016 # Assertions contain literal Markdown code spans.
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
*' bug list --whiteboard '*)
  product=ReleaseDemo
  if [[ -n ${FAKE_DISCOVERY_STATE:-} ]]; then
    count=0
    [[ ! -e $FAKE_DISCOVERY_STATE ]] || count=$(<"$FAKE_DISCOVERY_STATE")
    printf '%s\n' "$((count + 1))" >"$FAKE_DISCOVERY_STATE"
    if [[ ${FAKE_DISCOVERY_CHANGE:-0} -eq 1 && $count -gt 0 ]]; then
      product=ChangedDemo
    fi
  fi
  printf '%s\n' "{\"data\":[{\"id\":1,\"product\":\"$product\",\"whiteboard\":\"bzr-release-readiness-demo-v1 dependency\"},{\"id\":2,\"product\":\"$product\",\"whiteboard\":\"bzr-release-readiness-demo-v1 complete\"},{\"id\":3,\"product\":\"$product\",\"whiteboard\":\"bzr-release-readiness-demo-v1 release-blocker\"}]}"
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
  case ${FAKE_LINK_MODE:-default} in
  default)
    printf '%s\n' '{"data":[{"id":1,"relation":"depends_on","status":"NEW"}]}'
    ;;
  resolved)
    printf '%s\n' '{"data":[{"id":1,"relation":"depends_on","status":"RESOLVED"}]}'
    ;;
  unknown-null)
    printf '%s\n' '{"data":[{"id":1,"relation":"depends_on","status":null}]}'
    ;;
  unknown-missing)
    printf '%s\n' '{"data":[{"id":1,"relation":"depends_on"}]}'
    ;;
  omitted)
    printf '%s\n' '{"data":[]}'
    ;;
  link-only)
    printf '%s\n' '{"data":[{"id":7,"relation":"depends_on","status":"NEW"}]}'
    ;;
  *)
    printf 'unexpected FAKE_LINK_MODE: %s\n' "$FAKE_LINK_MODE" >&2
    exit 1
    ;;
  esac
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
  case ${FAKE_PRODUCT_MODE:-default} in
  default)
    printf '%s\n' '{"data":[{"id":1,"summary":"dependency","status":"NEW","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 dependency","depends_on":[]},{"id":2,"summary":"complete","status":"RESOLVED","priority":"Highest","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[]},{"id":3,"summary":"release root","status":"NEW","priority":"Highest","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 release-blocker","depends_on":[1]}]}'
    ;;
  unknown-status)
    printf '%s\n' '{"data":[{"id":1,"summary":"null status","status":null,"priority":"Highest","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 dependency","depends_on":[]},{"id":2,"summary":"missing status","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[]},{"id":3,"summary":"malformed status","status":7,"priority":"Highest","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 release-blocker","depends_on":[1]}]}'
    ;;
  unknown-time)
    printf '%s\n' '{"data":[{"id":1,"summary":"null timestamp","status":"NEW","priority":"Normal","last_change_time":null,"whiteboard":"bzr-release-readiness-demo-v1 dependency","depends_on":[]},{"id":2,"summary":"missing timestamp","status":"NEW","priority":"Normal","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[]},{"id":3,"summary":"malformed timestamp","status":"NEW","priority":"Normal","last_change_time":"not-a-time","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[1]}]}'
    ;;
  no-blocker)
    printf '%s\n' '{"data":[{"id":1,"summary":"dependency","status":"NEW","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 dependency","depends_on":[]},{"id":2,"summary":"complete","status":"RESOLVED","priority":"Highest","last_change_time":"2030-07-01T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[]},{"id":3,"summary":"release root","status":"NEW","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"bzr-release-readiness-demo-v1 complete","depends_on":[1]}]}'
    ;;
  asymmetric-blocker)
    printf '%s\n' '{"data":[{"id":1,"summary":"priority blocker","status":"NEW","priority":"Highest","last_change_time":"2030-08-29T00:00:00Z","depends_on":[]},{"id":2,"summary":"whiteboard blocker","status":"NEW","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"release-blocker","depends_on":[]},{"id":3,"summary":"complete root","status":"RESOLVED","last_change_time":"2030-08-29T00:00:00Z","depends_on":[]}]}'
    ;;
  missing-link-target)
    printf '%s\n' '{"data":[{"id":1,"summary":"visible work","status":"NEW","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"ordinary","depends_on":[]},{"id":2,"summary":"complete","status":"RESOLVED","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"ordinary","depends_on":[]},{"id":3,"summary":"release root","status":"NEW","priority":"Highest","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"release-blocker","depends_on":[7]}]}'
    ;;
  link-only-target)
    printf '%s\n' '{"data":[{"id":1,"summary":"visible work","status":"NEW","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"ordinary","depends_on":[]},{"id":2,"summary":"complete","status":"RESOLVED","priority":"Normal","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"ordinary","depends_on":[]},{"id":3,"summary":"release root","status":"NEW","priority":"Highest","last_change_time":"2030-08-29T00:00:00Z","whiteboard":"release-blocker","depends_on":[]}]}'
    ;;
  *)
    printf 'unexpected FAKE_PRODUCT_MODE: %s\n' "$FAKE_PRODUCT_MODE" >&2
    exit 1
    ;;
  esac
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
command=
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  if [[ ${args[i]} == -c ]]; then
    command=${args[i + 1]}
  fi
done
for output; do :; done
if [[ ${FAKE_ASCIINEMA_RUN_COMMAND:-0} -eq 1 ]]; then
  bash -c "$command" >"${FAKE_DRIVE_STDOUT:?}"
fi
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

assert_report_contains() {
  local report=$1
  local expected=$2
  grep -Fq -- "$expected" "$report" || {
    printf 'missing report contract in %s: %s\n' "$report" "$expected" >&2
    exit 1
  }
}

assert_report_excludes() {
  local report=$1
  local forbidden=$2
  if grep -Fq -- "$forbidden" "$report"; then
    printf 'forbidden report text in %s: %s\n' "$report" "$forbidden" >&2
    exit 1
  fi
}

run_helper_case() {
  local name=$1
  local product_mode=$2
  local link_mode=$3
  rm -f "$work/$name-date-state"
  (
    cd "$work"
    env "${common_env[@]}" FAKE_DATE_STATE="$work/$name-date-state" \
      FAKE_PRODUCT_MODE="$product_mode" FAKE_LINK_MODE="$link_mode" \
      bash "$sandbox/tools/run-release-readiness-demo.sh" demo \
      bzr-release-readiness-demo-v1 3 ReleaseDemo \
      "$work/$name-report.md" "$work/$name-trace.jsonl"
  )
}

rm -f "$work/date-state"
(
  cd "$work"
  env "${common_env[@]}" FAKE_DATE_STATE="$work/date-state" \
    bash "$sandbox/tools/run-release-readiness-demo.sh" demo \
    bzr-release-readiness-demo-v1 3 ReleaseDemo \
    "$work/timed-report.md" "$work/timed-trace.jsonl"
)
grep -Fq 'Generated: 2030-08-30T00:00:00Z' "$work/timed-report.md"
grep -Fq 'collection started 2030-08-30T00:00:05Z and ended 2030-08-30T00:00:10Z' \
  "$work/timed-report.md"
grep -Fq 'changed before 2030-07-31T00:00:00Z' "$work/timed-report.md"
grep -Fq '**Fact:** Ownership check: N/A (not selected).' "$work/timed-report.md"
grep -Fq '**Fact:** History/regression check: N/A (not selected); no history read was issued.' \
  "$work/timed-report.md"

jq -e '
  all(.[]; type == "object" and (.label | type) == "string" and
    (.argv | type) == "array" and .argv[0:4] ==
    ["bzr", "--server", "<server-profile>", "--json"]) and
  ([.[].label] | length) == ([.[].label] | unique | length) and
  any(.[]; .label == "marker-discovery" and .argv[4:6] == ["bug", "list"] and
    any(.argv[]; . == "bzr-release-readiness-demo-v1")) and
  ([.[] | select(.argv[4:6] == ["query", "show"])] | length) == 2 and
  any(.[]; .label == "product-scope" and
    .argv[4:7] == ["bug", "list", "--product"] and
    (.argv | index("id,summary,status,priority,depends_on,last_change_time,whiteboard") != null)) and
  any(.[]; .label == "dependency-links" and .argv[4:6] == ["bug", "links"]) and
  all(.[]; .argv[4:6] != ["bug", "history"])
' < <(jq -s . "$work/timed-trace.jsonl") >/dev/null
jq -r '"[\(.label)] \(.argv | join(" "))"' "$work/timed-trace.jsonl" \
  >"$work/trace-commands"
# shellcheck disable=SC2016 # The sed addresses are literal Markdown fences.
sed -n '/^```text$/,/^```$/p' "$work/timed-report.md" |
  sed '1d;$d' >"$work/report-commands"
cmp "$work/trace-commands" "$work/report-commands"
assert_report_contains "$work/timed-report.md" \
  '1/3 visible bugs are known to match a configured blocker. Bounded sample: #3. Source: `product-scope`.'
assert_report_contains "$work/timed-report.md" \
  '1/3 visible bugs are known stale under the stated assumptions. Bounded sample: #3. Source: `product-scope`.'
assert_report_contains "$work/timed-report.md" \
  '1/1 visible outgoing dependencies are known unresolved. Bounded sample: #1. Sources: `product-scope`, `dependency-links`.'
grep -Fq 'Blocker IDs: #3. Stale IDs: #3. Dependency-risk IDs: #1.' \
  "$work/timed-report.md"
grep -Fq 'whether dependency #1 must close before the release proceeds.' \
  "$work/timed-report.md"

run_helper_case unknown-status unknown-status unknown-null
assert_report_contains "$work/unknown-status-report.md" '**Assessment:** indeterminate.'
assert_report_contains "$work/unknown-status-report.md" \
  '0/3 visible bugs are known to match a configured blocker.'
assert_report_contains "$work/unknown-status-report.md" \
  '3/3 visible bugs have unknown blocker evidence. Bounded sample: #1, #2, #3. Source: `product-scope`.'
assert_report_contains "$work/unknown-status-report.md" \
  '3/3 visible bugs have unknown stale evidence. Bounded sample: #1, #2, #3. Source: `product-scope`.'
assert_report_contains "$work/unknown-status-report.md" \
  '1/1 visible outgoing dependencies have unknown or conflicting evidence. Bounded sample: #1. Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/unknown-status-report.md" \
  'Unknown blocker IDs: #1, #2, #3. Unknown stale IDs: #1, #2, #3. Unknown dependency-risk IDs: #1.'

run_helper_case unknown-time unknown-time unknown-missing
assert_report_contains "$work/unknown-time-report.md" \
  '0/3 visible bugs are known stale under the stated assumptions.'
assert_report_contains "$work/unknown-time-report.md" \
  '3/3 visible bugs have unknown stale evidence. Bounded sample: #1, #2, #3. Source: `product-scope`.'
assert_report_contains "$work/unknown-time-report.md" \
  '1/1 visible outgoing dependencies have unknown or conflicting evidence. Bounded sample: #1. Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/unknown-time-report.md" \
  'Unknown stale IDs: #1, #2, #3. Unknown dependency-risk IDs: #1.'

run_helper_case no-blocker no-blocker default
assert_report_contains "$work/no-blocker-report.md" \
  'No visible bug is known to match a configured blocker. Source: `product-scope`.'
assert_report_contains "$work/no-blocker-report.md" \
  'Decide whether dependency #1 must close before the release proceeds.'
assert_report_excludes "$work/no-blocker-report.md" '(none) is open'
assert_report_excludes "$work/no-blocker-report.md" 'whether blocker (none)'

run_helper_case no-dependency default resolved
assert_report_contains "$work/no-dependency-report.md" \
  'No visible outgoing dependency is known unresolved. Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/no-dependency-report.md" \
  'Decide whether blocker #3 can be cleared before the release proceeds.'
assert_report_excludes "$work/no-dependency-report.md" 'dependency (none)'
assert_report_excludes "$work/no-dependency-report.md" '(none) must close'
assert_report_excludes "$work/no-dependency-report.md" 'It affects #3.'

run_helper_case asymmetric-blocker asymmetric-blocker resolved
assert_report_contains "$work/asymmetric-blocker-report.md" \
  '2/3 visible bugs are known to match a configured blocker. Bounded sample: #1, #2. Source: `product-scope`.'
assert_report_contains "$work/asymmetric-blocker-report.md" \
  '0/3 visible bugs have unknown blocker evidence. Bounded sample: (none). Source: `product-scope`.'
assert_report_contains "$work/asymmetric-blocker-report.md" \
  'Known blocker IDs #1, #2 are open and match at least one configured blocker rule.'
assert_report_contains "$work/asymmetric-blocker-report.md" \
  'Unknown blocker IDs: (none).'

run_helper_case missing-link-target missing-link-target omitted
assert_report_contains "$work/missing-link-target-report.md" \
  '0/1 visible outgoing dependencies are known unresolved. Bounded sample: (none). Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/missing-link-target-report.md" \
  '1/1 visible outgoing dependencies have unknown or conflicting evidence. Bounded sample: #7. Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/missing-link-target-report.md" \
  'Unknown dependency-risk IDs: #7.'
assert_report_contains "$work/missing-link-target-report.md" \
  'Resolve the unknown selected evidence before relying on the affected checks.'

run_helper_case link-only-target link-only-target link-only
assert_report_contains "$work/link-only-target-report.md" \
  '0/1 visible outgoing dependencies are known unresolved. Bounded sample: (none). Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/link-only-target-report.md" \
  '1/1 visible outgoing dependencies have unknown or conflicting evidence. Bounded sample: #7. Sources: `product-scope`, `dependency-links`.'
assert_report_contains "$work/link-only-target-report.md" \
  'Unknown dependency-risk IDs: #7.'
assert_report_contains "$work/link-only-target-report.md" \
  'Resolve the unknown selected evidence before relying on the affected checks.'
assert_report_contains "$work/link-only-target-report.md" \
  'Dependency IDs are only classified when `product-scope` and `dependency-links` agree on the edge and the links record has a valid status.'

cat >"$work/fake-release-helper" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '# test report\n' >"$5"
EOF
chmod +x "$work/fake-release-helper"
env RELEASE_READINESS_DEMO_HELPER="$work/fake-release-helper" \
  RELEASE_READINESS_DEMO_MARKER=bzr-release-readiness-demo-v1 \
  RELEASE_READINESS_DEMO_REPORT="$work/driver-report.md" \
  RELEASE_READINESS_DEMO_ROOT=3 \
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

rm -f "$work/discovery-state" "$work/agg-called"
if env "${common_env[@]}" FAKE_CAST_CONTENT=verified-cast \
  FAKE_ASCIINEMA_RUN_COMMAND=1 FAKE_DRIVE_STDOUT="$work/changed-driver.stdout" \
  FAKE_DISCOVERY_STATE="$work/discovery-state" FAKE_DISCOVERY_CHANGE=1 \
  FAKE_DATE_STATE="$work/date-state" \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/changed.stdout" 2>"$work/changed.stderr"; then
  printf 'recorder identity failure: changed second discovery was accepted\n' >&2
  exit 1
fi
cmp "$work/cast.before" "$tracked_cast" || {
  printf 'recorder identity failure: mismatched evidence replaced the cast\n' >&2
  exit 1
}
cmp "$work/gif.before" "$tracked_gif" || {
  printf 'recorder identity failure: mismatched evidence replaced the GIF\n' >&2
  exit 1
}
[[ ! -e $work/agg-called ]] || {
  printf 'recorder identity failure: mismatched evidence reached the renderer\n' >&2
  exit 1
}

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
