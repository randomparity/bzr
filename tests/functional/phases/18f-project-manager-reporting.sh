# 18f-project-manager-reporting
# Sourced by run-tests.sh after the shared Bugzilla fixture exists.
# shellcheck shell=bash

echo "── Phase 18f: Project-manager reporting workflow ───────────"

_PM_MARKER=$(unique_name bzr-pm-demo-v1)
_PM_QUERY=pm-report-fixture
# Isolate URL-to-profile matching from the deliberately duplicated 127.0.0.1
# profiles in phase 18d. The old `alt` fixture is the only earlier localhost
# profile and is no longer used after phase 1.
run_bzr config remove-server alt
_PM_ALT_REMOVED=$BZR_EXIT
_PM_PUBLIC_URL=${BZ_URL/127.0.0.1/localhost}
run_bzr config set-server pm-report-public --url "$_PM_PUBLIC_URL" --api rest
_PM_PROFILE_OK=$BZR_EXIT

_PM_CREATE=(--product FuncTestProd --component Backend --op-sys Linux
  --platform PC --description "project-manager reporting fixture")
_PM_BLOCKER=$(make_bug "${_PM_CREATE[@]}" --summary "Parser rollout" \
  --whiteboard "$_PM_MARKER blocker blocked: owner needed")
_PM_QA=$(make_bug "${_PM_CREATE[@]}" --summary "QA validation" \
  --whiteboard "$_PM_MARKER qa verification underway")
_PM_DOCS=$(make_bug "${_PM_CREATE[@]}" --summary "Documentation readiness" \
  --whiteboard "$_PM_MARKER docs ready for review")

test_begin "pm-fixture-records-mutable-whiteboard-and-durable-comment" "PM fixture records mutable whiteboard and durable comment"
if [[ -n $_PM_BLOCKER && -n $_PM_QA && -n $_PM_DOCS ]]; then
    run_bzr bug update "$_PM_BLOCKER" --status IN_PROGRESS
    [[ $BZR_EXIT -eq 0 ]] || _PM_BLOCKER=""
fi
if [[ -n $_PM_BLOCKER ]]; then
    run_bzr comment add "$_PM_BLOCKER" --body "$_PM_MARKER durable weekly update"
    if assert_success; then
        run_bzr comment list "$_PM_BLOCKER" --fields id,bug_id,text,creation_time
        if assert_success &&
            assert_json 'map(select(.text | contains("durable weekly update"))) | length' "1"; then
            test_pass
        fi
    fi
fi

test_begin "pm-custom-search-saves-and-paginates-projected-json" "PM Custom Search saves and paginates projected JSON"
_PM_URL="${_PM_PUBLIC_URL}/buglist.cgi?product=FuncTestProd&f1=status_whiteboard&o1=substring&v1=${_PM_MARKER}&query_format=advanced"
run_bzr query save "$_PM_QUERY" --from-url "$_PM_URL"
if [[ $_PM_ALT_REMOVED -eq 0 && $_PM_PROFILE_OK -eq 0 ]] && assert_success; then
    RUST_LOG=bzr=warn run_bzr --api xmlrpc query run "$_PM_QUERY" \
        --fields summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
        --limit 1 --paginate
    if assert_success && assert_json_array_length '.' 3 &&
        assert_json '.[0] | (has("id") | not) and has("summary") and has("status") and has("whiteboard")' "true" &&
        assert_json 'map(.summary) | sort | join(",")' "Documentation readiness,Parser rollout,QA validation" &&
        assert_json 'map(select(.whiteboard | contains("blocked: owner needed"))) | length' "1"; then
        _PM_REST_WARNING_COUNT=$(awk '
          index($0, "query contains raw URL parameters that require REST API") { count++ }
          END { print count + 0 }
        ' "$BZR_STDERR")
        if [[ $_PM_REST_WARNING_COUNT -eq 1 ]]; then
            test_pass
        else
            test_fail "raw-parameter REST fallback warning count = $_PM_REST_WARNING_COUNT, expected 1"
        fi
    fi
fi

test_begin "pm-custom-search-emits-bare-projected-ndjson-rows" "PM Custom Search emits bare projected NDJSON rows"
RUST_LOG=bzr=warn run_bzr_raw --api rest --output ndjson bug search --from-url "$_PM_URL" \
    --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
    --limit 1 --paginate
if assert_success && assert_ndjson_line_count 3 &&
    jq -s -e 'length == 3 and
      (map(.summary) | sort) == ["Documentation readiness", "Parser rollout", "QA validation"]' \
      "$BZR_STDOUT" >/dev/null &&
    assert_stderr_not_contains "query contains raw URL parameters that require REST API"; then
    test_pass
fi

run_bzr query delete "$_PM_QUERY"
[[ $BZR_EXIT -eq 0 ]] || test_fail "PM report fixture query cleanup failed"
unset _PM_MARKER _PM_QUERY _PM_CREATE _PM_BLOCKER _PM_QA _PM_DOCS _PM_REST_WARNING_COUNT
unset _PM_ALT_REMOVED _PM_PUBLIC_URL _PM_PROFILE_OK _PM_URL

echo ""
