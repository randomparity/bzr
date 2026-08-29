# 18f-project-manager-reporting
# Sourced by run-tests.sh after the shared Bugzilla fixture exists.
# shellcheck shell=bash

echo "── Phase 18f: Project-manager reporting workflow ───────────"

_PM_MARKER=$(unique_name pm-report)
_PM_QUERY=pm-report-fixture
# Isolate URL-to-profile matching from the deliberately duplicated 127.0.0.1
# profiles in phase 18d. The old `alt` fixture is the only earlier localhost
# profile and is no longer used after phase 1.
run_bzr config remove-server alt
_PM_ALT_REMOVED=$BZR_EXIT
_PM_PUBLIC_URL=${BZ_URL/127.0.0.1/localhost}
run_bzr config set-server pm-report-public --url "$_PM_PUBLIC_URL" --api rest
_PM_PROFILE_OK=$BZR_EXIT

test_begin "125a. PM fixture records mutable whiteboard and durable comment"
run_bzr bug update "$BUG1" --whiteboard "$_PM_MARKER blocked: owner:program-manager"
if assert_success; then
    run_bzr comment add "$BUG1" --body "$_PM_MARKER durable weekly update"
    if assert_success; then
        run_bzr comment list "$BUG1" --fields id,bug_id,text,creation_time
        if assert_success &&
            assert_json 'map(select(.text | contains("durable weekly update"))) | length' "1"; then
            test_pass
        fi
    fi
fi

test_begin "125b. PM Custom Search saves and paginates projected JSON"
_PM_URL="${_PM_PUBLIC_URL}/buglist.cgi?product=FuncTestProd&f1=status_whiteboard&o1=substring&v1=${_PM_MARKER}&query_format=advanced"
run_bzr query save "$_PM_QUERY" --from-url "$_PM_URL"
if [[ $_PM_ALT_REMOVED -eq 0 && $_PM_PROFILE_OK -eq 0 ]] && assert_success; then
    run_bzr query run "$_PM_QUERY" \
        --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
        --paginate
    if assert_success && assert_json_array_min_length '.' 1 &&
        assert_json '.[0] | has("id") and has("summary") and has("status") and has("whiteboard")' "true" &&
        assert_json 'map(select(.whiteboard | contains("owner:program-manager"))) | length' "1"; then
        test_pass
    fi
fi

test_begin "125c. PM Custom Search emits bare projected NDJSON rows"
run_bzr_raw --output ndjson bug search --from-url "$_PM_URL" \
    --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
    --paginate
if assert_success && assert_ndjson_line_count 1 &&
    jq -e 'has("id") and has("summary") and has("whiteboard") and
      (.whiteboard | contains("owner:program-manager"))' "$BZR_STDOUT" >/dev/null; then
    test_pass
fi

run_bzr query delete "$_PM_QUERY"
[[ $BZR_EXIT -eq 0 ]] || test_fail "PM report fixture query cleanup failed"
unset _PM_MARKER _PM_QUERY _PM_ALT_REMOVED _PM_PUBLIC_URL _PM_PROFILE_OK _PM_URL

echo ""
