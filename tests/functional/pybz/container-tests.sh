#!/bin/bash
# Focused fixtures for the python-bugzilla comparison sidecar.
# The sourced phase calls fixture functions dynamically; wrapper output is captured in a subshell.
# shellcheck disable=SC1090,SC2030,SC2031,SC2034,SC2329
set -euo pipefail

PYBZ_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/functional/lib.sh
source "$PYBZ_DIR/../lib.sh"

assert_equals() {
    local expected="$1"
    local actual="$2"
    local label="$3"

    if [[ $actual != "$expected" ]]; then
        printf 'expected %s to be %s, got %s\n' "$label" "$expected" "$actual" >&2
        return 1
    fi
    return 0
}

run_expected_gap_fixture() {
    local summary
    local result_output
    summary=$(mktemp)
    result_output=$(mktemp)
    trap 'rm -f "$summary" "$result_output"' RETURN
    export GITHUB_STEP_SUMMARY="$summary"
    TEST_ID_PREFIX=compare
    BZ_VERSION=bz50

    CURRENT_TEST_GROUP="20-pybz"
    {
        test_begin "expected-gap" "expected client gap"
        test_fail "known comparison difference"
        expect_gap 666
    } >"$result_output"
    assert_equals \
        '  TEST  [compare/20-pybz/expected-gap] expected client gap ... GAP (#666)' \
        "$(<"$result_output")" "expected-gap terminal output"
    assert_equals 0 "$PASS_COUNT" "pass count"
    assert_equals 0 "$FAIL_COUNT" "fail count"
    assert_equals 0 "$SKIP_COUNT" "skip count"
    assert_equals 1 "$GAP_COUNT" "gap count"
    if ! test_summary; then
        printf 'expected-gap-only summary failed\n' >&2
        return 1
    fi
    assert_equals $'## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz50 | 0 | 0 | 0 | 1 |' \
        "$(<"$summary")" "comparison GitHub summary"

    if expect_gap 667; then
        printf 'expected gap was accepted twice\n' >&2
        return 1
    fi
    if expect_gap not-a-decimal-issue; then
        printf 'non-decimal issue was accepted\n' >&2
        return 1
    fi

    PASS_COUNT=0
    FAIL_COUNT=0
    SKIP_COUNT=0
    GAP_COUNT=0
    CURRENT_TEST_GROUP="20-pybz"
    {
        test_begin "stale-gap" "stale expected client gap"
        test_pass
        expect_gap 666
    } >"$result_output"
    assert_equals \
        '  TEST  [compare/20-pybz/stale-gap] stale expected client gap ... FAIL  (expected gap issue #666 appears resolved)' \
        "$(<"$result_output")" "stale-gap terminal output"
    assert_equals 0 "$PASS_COUNT" "stale pass count"
    assert_equals 1 "$FAIL_COUNT" "stale fail count"
    assert_equals 0 "$SKIP_COUNT" "stale skip count"
    assert_equals 0 "$GAP_COUNT" "stale gap count"
    if test_summary; then
        printf 'stale expected gap summary unexpectedly passed\n' >&2
        return 1
    fi
    return 0
}

run_summary_fixture() {
    local summary
    local ordinary_output
    summary=$(mktemp)
    trap 'rm -f "$summary"' RETURN
    export GITHUB_STEP_SUMMARY="$summary"

    PASS_COUNT=1
    FAIL_COUNT=0
    SKIP_COUNT=2
    GAP_COUNT=3
    TEST_ID_PREFIX=''
    BZ_VERSION=bz50
    ordinary_output=$(test_summary)
    assert_equals $'\n════════════════════════════════════════════════════════════\n  PASSED: 1  FAILED: 0  SKIPPED: 2\n  TOTAL:  3\n════════════════════════════════════════════════════════════' \
        "$ordinary_output" "ordinary terminal summary"
    assert_equals '' "$(<"$summary")" "ordinary GitHub summary"

    : >"$summary"
    TEST_ID_PREFIX=compare
    PASS_COUNT=1
    FAIL_COUNT=0
    SKIP_COUNT=2
    GAP_COUNT=3
    for BZ_VERSION in bz50 bz52 bz53; do
        test_summary >/dev/null
    done
    assert_equals $'## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz50 | 1 | 0 | 2 | 3 |\n\n## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz52 | 1 | 0 | 2 | 3 |\n\n## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz53 | 1 | 0 | 2 | 3 |' \
        "$(<"$summary")" "multi-version comparison GitHub summary"
    return 0
}

run_product_normalization_fixture() (
    local fixture_output
    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT

    PASS_COUNT=0
    FAIL_COUNT=0
    SKIP_COUNT=0
    GAP_COUNT=0
    TEST_ID_PREFIX=compare
    CURRENT_TEST_GROUP=00-products
    BZ_URL=http://127.0.0.1

    run_bzr() {
        printf '%s\n' \
            '[{"name":"Beta"},{"name":""},{"name":"Alpha"},{"name":"Alpha"}]' \
            >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT=0
    }
    run_pybz() {
        printf '\nAlpha\nBeta\nBeta\n' >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT=0
    }

    # shellcheck source=tests/functional/compare/00-products.sh
    source "$PYBZ_DIR/../compare/00-products.sh" >"$fixture_output"
    assert_equals 1 "$PASS_COUNT" "normalized product pass count"
    assert_equals 0 "$FAIL_COUNT" "normalized product fail count"
    assert_equals $'Alpha\nBeta' "$(<"$COMPARE_EXCHANGE_DIR/bzr-product-names")" \
        "normalized bzr product names"
    assert_equals $'Alpha\nBeta' "$(<"$COMPARE_EXCHANGE_DIR/pybz-product-names")" \
        "normalized python-bugzilla product names"
)

run_sidecar_wrapper_fixture() (
    local invoked
    invoked=$(mktemp)
    trap 'rm -f "$invoked"' EXIT
    BZ_VERSION=bz50 PYBZ_RUNTIME=fake_wrapper_runtime
    fake_wrapper_runtime() {
        printf '%s\n' "${*:3}" >"$invoked"
        printf '{"schema_version":"3.0.0","data":{"fixed":true}}\n'
        printf 'fixed stderr\n' >&2
        [[ $3 == bugzilla ]] && return 17
        return 19
    }

    run_pybz info
    assert_equals 'bugzilla info' "$(<"$invoked")" "fixed CLI command"
    assert_equals $'{\n  "fixed": true\n}' "$(<"$BZR_STDOUT")" "CLI projected stdout"
    assert_equals '{"schema_version":"3.0.0","data":{"fixed":true}}' \
        "$(<"$BZR_STDOUT_RAW")" "CLI raw stdout"
    assert_equals 'fixed stderr' "$(<"$BZR_STDERR")" "CLI stderr"
    assert_equals 17 "$BZR_EXIT" "CLI exit"

    run_pybz_adapter view /work/compare/in.json /work/compare/out.json
    assert_equals \
        'python /work/compare/bug-lifecycle.py view /work/compare/in.json /work/compare/out.json' \
        "$(<"$invoked")" "fixed adapter command"
    assert_equals $'{\n  "fixed": true\n}' "$(<"$BZR_STDOUT")" "adapter projected stdout"
    assert_equals '{"schema_version":"3.0.0","data":{"fixed":true}}' \
        "$(<"$BZR_STDOUT_RAW")" "adapter raw stdout"
    assert_equals 'fixed stderr' "$(<"$BZR_STDERR")" "adapter stderr"
    assert_equals 19 "$BZR_EXIT" "adapter exit"
)

run_lifecycle_phase_fixture() (
    local phase="$PYBZ_DIR/../compare/01-bug-lifecycle.sh"
    local fixture_output
    local control_failures=0
    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT
    if [[ ! -r $phase ]]; then
        printf 'missing lifecycle phase IDs: compare/01-bug-lifecycle/{create,query,update,view,history}\n' >&2
        return 1
    fi
    umask 077
    PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
    TEST_ID_PREFIX=compare CURRENT_TEST_GROUP=01-bug-lifecycle BZ_VERSION=bz50
    BZ_URL=http://127.0.0.1 BZR_COMPARE_API_KEY=fixture-secret
    COMPARE_ADMIN_EMAIL=admin@test.bzr PYBZ_RUNTIME=fake_lifecycle_runtime
    LIFECYCLE_BZR_ARGS="$COMPARE_EXCHANGE_DIR/bzr.args"
    sleep() { :; }

    seed_server_saved_search() {
        [[ $1 == admin@test.bzr && -n $2 && ${#2} -le 64 &&
            $3 =~ ^[1-9][0-9]*$ && $4 =~ ^[1-9][0-9]*$ ]]
    }

    fixture_bug() {
        local id="$1" summary="$2"
        local initial_summary="$LIFECYCLE_BZR_SUMMARY" severity=normal priority=Normal url='' whiteboard=''
        [[ $id -eq 41 ]] || initial_summary="$LIFECYCLE_PYBZ_SUMMARY"
        if [[ ${LIFECYCLE_UPDATED:-0} -eq 1 ]]; then
            summary="$LIFECYCLE_UPDATED_SUMMARY" url="$LIFECYCLE_URL"
            whiteboard="$LIFECYCLE_WHITEBOARD" severity=major priority=High
        fi
        case "${LIFECYCLE_STATE_NOOP_FIELD:-}" in
        summary) summary="$initial_summary" ;;
        url) url='' ;;
        whiteboard) whiteboard='' ;;
        severity) severity=normal ;;
        priority) priority=Normal ;;
        esac
        jq -cn --argjson id "$id" --arg summary "$summary" --arg severity "$severity" \
            --arg priority "$priority" --arg url "$url" --arg whiteboard "$whiteboard" \
            '{id:$id,product:"TestProduct",component:"TestComponent",version:"unspecified",
              summary:$summary,op_sys:"Linux",platform:"PC",severity:$severity,priority:$priority,
              status:"NEW",resolution:"",url:$url,whiteboard:$whiteboard,
              cc:[],keywords:[]}'
    }
    reset_lifecycle_fixture() {
        PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
        SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 LIFECYCLE_UPDATED=0
        LIFECYCLE_PYBZ_TAGGED=0 LIFECYCLE_GENERIC_CREATE_COUNT=0 LIFECYCLE_GENERIC_UPDATED=0
        LIFECYCLE_GENERIC_BZR_UPDATED=0
        : >"$LIFECYCLE_BZR_ARGS"
    }
    run_lifecycle_failure_control() {
        local flag="$1" capability="$2" label="$3" value="${4:-1}"

        reset_lifecycle_fixture
        printf -v "$flag" %s "$value"
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag"
        if [[ $FAIL_COUNT -eq 0 ]] ||
            ! grep -Fq "[compare/01-bug-lifecycle/${capability}] ${label} ... FAIL" \
                "$fixture_output"; then
            printf '%s control %s unexpectedly passed\n' "$capability" "$flag" >&2
            return 1
        fi
        printf 'controlled red: %s %s=%s\n' "$capability" "$flag" "$value"
    }
    run_partial_stale_gap_control() {
        local flag="$1" issue="$2" capability="$3" label="$4"

        reset_lifecycle_fixture
        LIFECYCLE_STALE_GAPS=1
        printf -v "$flag" 1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag" LIFECYCLE_STALE_GAPS
        if ! grep -Fq \
            "[compare/01-bug-lifecycle/${capability}] ${label} ... GAP (#${issue})" \
            "$fixture_output"; then
            printf '%s partial control %s did not remain a gap\n' "$capability" "$flag" >&2
            return 1
        fi
        printf 'controlled red: %s %s=1\n' "$capability" "$flag"
    }
    run_noop_stale_gap_control() {
        reset_lifecycle_fixture
        LIFECYCLE_NOOP_STALE_GAPS=1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset LIFECYCLE_NOOP_STALE_GAPS
        if [[ $FAIL_COUNT -ne 0 || $GAP_COUNT -ne 5 ]] ||
            ! grep -Fq '[compare/01-bug-lifecycle/update-options] comment tags and minor update ... GAP (#672)' \
                "$fixture_output" ||
            ! grep -Fq '[compare/01-bug-lifecycle/bug-tags] personal bug tags ... GAP (#680)' \
                "$fixture_output"; then
            printf 'no-op stale mutation controls did not remain gaps\n' >&2
            return 1
        fi
    }
    run_bzr() {
        local args=" $* " id=41 summary="$LIFECYCLE_BZR_SUMMARY" value
        printf '%s\n' "$*" >>"$LIFECYCLE_BZR_ARGS"
        : >"$BZR_STDOUT"
        [[ $args == *" 42 "* ]] && id=42 && summary="$LIFECYCLE_PYBZ_SUMMARY"
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == view-bzr &&
            ${LIFECYCLE_WRONG_ID_TARGET:-} == bzr-view ]]; then id=99; fi
        if [[ ${LIFECYCLE_UPDATED:-0} -eq 1 ]]; then summary="$LIFECYCLE_UPDATED_SUMMARY"; fi
        if [[ ${LIFECYCLE_NOOP_STALE_GAPS:-0} -eq 1 &&
            ( $args == *" --comment-tag "* || $args == *" bug tag "* ) ]]; then
            printf '{}\n' >"$BZR_STDOUT"
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=0
            return 0
        fi
        if [[ ${LIFECYCLE_NOOP_STALE_GAPS:-0} -eq 1 && $args == *" bug list "* &&
            $args == *" --tag "* ]]; then
            if [[ $args == *" --tag $LIFECYCLE_BUG_TAG "* ]]; then
                printf '[{"id":42}]\n' >"$BZR_STDOUT"
            else
                printf '[]\n' >"$BZR_STDOUT"
            fi
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=0
            return 0
        fi
        if [[ ${LIFECYCLE_STALE_GAPS:-0} -eq 1 && $args == *" bug list "* &&
            $args == *" --tag "* ]]; then
            printf '[{"id":42}]\n' >"$BZR_STDOUT"
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=0
            return 0
        fi
        if [[ ${LIFECYCLE_STALE_GAPS:-0} -ne 1 &&
            ${LIFECYCLE_NOOP_STALE_GAPS:-0} -ne 1 &&
            ( $args == *" --saved-search "* || $args == *" --field "* ||
                $args == *" --comment-tag "* || $args == *" --status-whiteboard-type "* ||
                $args == *" bug tag "* || $args == *" --tag "* ) ]]; then
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=2
            return 0
        fi
        if [[ ${LIFECYCLE_STALE_GAPS:-0} -eq 1 ]]; then
            case "$args" in
            *" --saved-search "*) printf '[{"id":41},{"id":42}]\n' >"$BZR_STDOUT" ;;
            *" --field whiteboard="*)
                if [[ $args == *" bug create "* ]]; then printf '{"id":46}\n' >"$BZR_STDOUT"
                else LIFECYCLE_GENERIC_BZR_UPDATED=1; printf '{}\n' >"$BZR_STDOUT"; fi
                ;;
            *" bug view 46 "*)
                value="$LIFECYCLE_FIELD_INITIAL"
                [[ ${LIFECYCLE_GENERIC_CREATE_NOOP:-0} -eq 0 ]] || value=''
                [[ ${LIFECYCLE_GENERIC_BZR_UPDATED:-0} -eq 0 ]] || value="$LIFECYCLE_FIELD_UPDATED"
                jq -cn --arg value "$value" '{id:46,whiteboard:$value}' >"$BZR_STDOUT"
                ;;
            *" --comment-tag "*)
                if [[ $args == *" --dry-run "* ]]; then
                    if [[ ${LIFECYCLE_MINOR_UPDATE_OMITTED:-0} -eq 1 ]]; then
                        printf '{"changes":{}}\n' >"$BZR_STDOUT"
                    else printf '{"changes":{"minor_update":true}}\n' >"$BZR_STDOUT"; fi
                else printf '{}\n' >"$BZR_STDOUT"; fi
                ;;
            *" bug list "*" --status-whiteboard-type equals "*) printf '[{"id":44}]\n' >"$BZR_STDOUT" ;;
            *" bug tag "*) printf '{}\n' >"$BZR_STDOUT" ;;
            *" bug list "*" --tag "*) printf '[{"id":42}]\n' >"$BZR_STDOUT" ;;
            *) : ;;
            esac
            if [[ -s $BZR_STDOUT ]]; then
                cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=0
                return 0
            fi
        fi
        case "$args" in
        *" bug create "*) printf '{"id":41}\n' >"$BZR_STDOUT" ;;
        *" bug list "*)
            if [[ $args == *" --summary "* && ${LIFECYCLE_QUERY_EMPTY:-0} -eq 1 ]]; then
                printf '[]\n' >"$BZR_STDOUT"
            elif [[ $args == *" --summary "* && ${LIFECYCLE_QUERY_COLLISION:-0} -eq 1 ]]; then
                fixture_bug 41 "$LIFECYCLE_BZR_SUMMARY" | jq -s '. + [.[0] + {id:40}]' \
                    >"$BZR_STDOUT"
            else
                fixture_bug 41 "$LIFECYCLE_BZR_SUMMARY" | jq -s . >"$BZR_STDOUT"
            fi
            ;;
        *" bug update "*) LIFECYCLE_UPDATED=1; printf '{}\n' >"$BZR_STDOUT" ;;
        *" bug view "*) fixture_bug "$id" "$summary" >"$BZR_STDOUT" ;;
        *" comment list "*)
            if [[ ${LIFECYCLE_STALE_GAPS:-0} -eq 1 ]]; then
                jq -cn --arg text "${LIFECYCLE_BZR_COMMENT:-$LIFECYCLE_COMMENT}" \
                    --arg tag "${LIFECYCLE_BZR_COMMENT_TAG:-$LIFECYCLE_COMMENT_TAG}" \
                    '[{text:$text,tags:[$tag]}]' >"$BZR_STDOUT"
            elif [[ ${LIFECYCLE_PYBZ_TAGGED:-0} -eq 1 && $args == *" 42 "* ]]; then
                jq -cn --arg text "$LIFECYCLE_COMMENT" --arg tag "$LIFECYCLE_COMMENT_TAG" \
                    '[{text:$text,tags:[$tag]}]' >"$BZR_STDOUT"
            else printf '[{"count":0,"text":"lifecycle description"}]\n' >"$BZR_STDOUT"; fi
            ;;
        *" bug history "*) jq -cn --arg old "$LIFECYCLE_BZR_SUMMARY" \
            --arg new "$LIFECYCLE_UPDATED_SUMMARY" --arg near "$LIFECYCLE_STEM [bzr] extra" \
            --arg omit "${LIFECYCLE_HISTORY_OMIT_FIELD:-}" \
            '[{field:"summary",old_value:$old,new_value:$new},
              {field:"summary",old_value:$near,new_value:"preserved"},
              {field:"url",old_value:"",new_value:"https://example.test/updated"},
              {field:"whiteboard",old_value:"",new_value:"updated"},
              {field:"severity",old_value:"normal",new_value:"major"},
              {field:"priority",old_value:"Normal",new_value:"High"}] |
              map(select(.field != $omit))' >"$BZR_STDOUT" ;;
        *)
            if [[ ${LIFECYCLE_STALE_GAPS:-0} -ne 1 ]]; then
                : >"$BZR_STDOUT"; cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"
                BZR_EXIT=2
                return 0
            fi
            return 2
            ;;
        esac
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"; : >"$BZR_STDERR"; BZR_EXIT=0
    }
    fake_lifecycle_runtime() {
        local operation="$5" output="$COMPARE_EXCHANGE_DIR/${7##*/}" result transport=FixtureXMLRPC
        local input="$COMPARE_EXCHANGE_DIR/${6##*/}" request value
        request=$(<"$input")
        case "$operation" in
        create) result='{"id":42}' ;;
        query)
            result=$(fixture_bug 42 "$LIFECYCLE_PYBZ_SUMMARY" | jq -s .)
            if [[ ${LIFECYCLE_QUERY_EMPTY:-0} -eq 1 && $request == *short_desc* ]]; then
                result='[]'
            elif [[ ${LIFECYCLE_QUERY_EXTRA:-0} -eq 1 && $request == *short_desc* ]]; then
                result=$(jq '. + [.[0] + {id:52}]' <<<"$result")
            elif [[ ${LIFECYCLE_QUERY_COLLISION:-0} -eq 1 && $request == *short_desc* ]]; then
                result=$(jq '. + [.[0] + {id:40}]' <<<"$result")
            fi
            ;;
        update) LIFECYCLE_UPDATED=1; result='{}' ;;
        view)
            result=$(fixture_bug 42 "$LIFECYCLE_UPDATED_SUMMARY")
            if [[ ${input##*/} == view.pybz.input.json &&
                ${LIFECYCLE_WRONG_ID_TARGET:-} == pybz-view ]]; then
                result=$(jq '.id=99' <<<"$result")
            fi
            [[ ${PYBZ_LIFECYCLE_MISMATCH:-0} -eq 0 ]] || result=$(jq '.priority="Wrong"' <<<"$result")
            ;;
        history)
            result=$(jq -cn --arg old "$LIFECYCLE_PYBZ_SUMMARY" \
                --arg new "$LIFECYCLE_UPDATED_SUMMARY" --arg near "$LIFECYCLE_STEM [bzr] extra" \
                '{bugs:[{id:42,history:[{changes:[{field_name:"summary",removed:$old,added:$new},
                  {field_name:"summary",removed:$near,added:"preserved"},
                  {field_name:"url",removed:"",added:"https://example.test/updated"},
                  {field_name:"whiteboard",removed:"",added:"updated"},
                  {field_name:"severity",removed:"normal",added:"major"},
                  {field_name:"priority",removed:"Normal",added:"High"}]}]}]}')
            if [[ ${LIFECYCLE_HISTORY_REVERSED:-0} -eq 1 ]]; then
                result=$(jq '.bugs[0].history[0].changes |= reverse' <<<"$result")
            fi
            if [[ -n ${LIFECYCLE_HISTORY_OMIT_FIELD:-} ]]; then
                result=$(jq --arg field "$LIFECYCLE_HISTORY_OMIT_FIELD" \
                    '.bugs[0].history[0].changes |= map(select(.field_name != $field))' <<<"$result")
            fi
            if [[ ${LIFECYCLE_WRONG_ID_TARGET:-} == history ]]; then
                result=$(jq '.bugs[0].id=99' <<<"$result")
            elif [[ ${LIFECYCLE_HISTORY_EXTRA:-0} -eq 1 ]]; then
                result=$(jq '.bugs += [(.bugs[0] | .id=52)]' <<<"$result")
            fi
            ;;
        saved_search) result='[{"id":41},{"id":42}]' ;;
        generic_fields)
            if [[ $(jq -r '.action' <<<"$request") == create ]]; then
                LIFECYCLE_GENERIC_CREATE_COUNT=${LIFECYCLE_GENERIC_CREATE_COUNT:-0}
                LIFECYCLE_GENERIC_CREATE_COUNT=$((LIFECYCLE_GENERIC_CREATE_COUNT + 1))
                result=$(jq -cn --argjson id "$((42 + LIFECYCLE_GENERIC_CREATE_COUNT))" '{id:$id}')
            else LIFECYCLE_GENERIC_UPDATED=1; result='{}'; fi
            ;;
        match_type) result='[{"id":44}]' ;;
        update_options) LIFECYCLE_PYBZ_TAGGED=1; result='{}'; transport=FixtureREST ;;
        bug_tags) result='{"bugs":[{"id":42}],"update":{}}'; transport=FixtureXMLRPC ;;
        *) return 2 ;;
        esac
        if [[ $operation == view && $(jq -r '.bug_id // 0' <<<"$request") == 43 ]]; then
            value="$LIFECYCLE_FIELD_INITIAL"
            [[ ${LIFECYCLE_GENERIC_UPDATED:-0} -eq 0 ]] || value="$LIFECYCLE_FIELD_UPDATED"
            result=$(jq -cn --arg value "$value" \
                '{id:43,whiteboard:$value}')
        fi
        if [[ $operation == query && $request == *status_whiteboard* ]]; then
            result='[{"id":44},{"id":45}]'
        fi
        jq -cn --arg transport "$transport" --argjson result "$result" \
            '{transport:$transport,result:$result}' >"$output"
        return 0
    }

    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    if [[ $FAIL_COUNT -ne 0 ]]; then cat "$fixture_output" >&2; fi
    assert_equals 5 "$PASS_COUNT" "lifecycle pass count"
    assert_equals 0 "$FAIL_COUNT" "lifecycle fail count"
    assert_equals 5 "$GAP_COUNT" "lifecycle gap count"
    for slug in create query update view history saved-search arbitrary-fields update-options \
        query-match-types bug-tags; do
        grep -Fq "compare/01-bug-lifecycle/$slug" "$fixture_output"
    done
    assert_equals \
        '[{"field":"summary","old_value":"'"$LIFECYCLE_STEM"'","new_value":"'"$LIFECYCLE_UPDATED_SUMMARY"'"},{"field":"summary","old_value":"'"$LIFECYCLE_STEM"' [bzr] extra","new_value":"preserved"},{"field":"url","old_value":"","new_value":"https://example.test/updated"},{"field":"whiteboard","old_value":"","new_value":"updated"},{"field":"severity","old_value":"normal","new_value":"major"},{"field":"priority","old_value":"Normal","new_value":"High"}]' \
        "$(jq -c . "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json")" \
        "exact-only ordered history normalization"
    if ! grep -Fq -- '--severity major' "$LIFECYCLE_BZR_ARGS" ||
        ! grep -Fq -- '--priority High' "$LIFECYCLE_BZR_ARGS" ||
        ! jq -e '.params.severity == "major"' \
            "$COMPARE_EXCHANGE_DIR/update-severity.pybz.input.json" >/dev/null ||
        ! jq -e '.params.priority == "High"' \
            "$COMPARE_EXCHANGE_DIR/update-priority.pybz.input.json" >/dev/null ||
        ! jq -se 'all(.[]; .severity == "major" and .priority == "High")' \
            "$COMPARE_EXCHANGE_DIR/update.bzr.normalized.json" \
            "$COMPARE_EXCHANGE_DIR/update.pybz.normalized.json" >/dev/null ||
        ! jq -se 'all(.[]; [.[] | select(.field == "severity" or .field == "priority")] ==
            [{field:"severity",old_value:"normal",new_value:"major"},
             {field:"priority",old_value:"Normal",new_value:"High"}])' \
            "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" \
            "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json" >/dev/null; then
        printf 'update did not persist and preserve ordered severity/priority transitions\n' >&2
        control_failures=$((control_failures + 1))
    fi
    _run_token=${LIFECYCLE_RUN_TOKEN:-}
    if [[ ! $_run_token =~ ^[0-9a-f]+-[0-9a-f]+-[0-9a-f]+$ || ${#_run_token} -gt 18 ||
        $LIFECYCLE_STEM != "bzr-pybz-lifecycle-${BZ_VERSION}-${_run_token}" ||
        $LIFECYCLE_SAVED_SEARCH != "lifecycle-${BZ_VERSION}-${_run_token}" ||
        $LIFECYCLE_COMMENT != "tagged-comment-${_run_token}" ||
        $LIFECYCLE_BUG_TAG != "bug-tag-${_run_token}" || ${#LIFECYCLE_COMMENT_TAG} -gt 24 ||
        ${#LIFECYCLE_BZR_COMMENT_TAG} -gt 24 ]]; then
        printf 'lifecycle values did not reuse one bounded high-entropy run token\n' >&2
        control_failures=$((control_failures + 1))
    fi

    if ! run_lifecycle_failure_control LIFECYCLE_QUERY_EMPTY query 'bug query'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_lifecycle_failure_control LIFECYCLE_QUERY_EXTRA query 'bug query'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_lifecycle_failure_control LIFECYCLE_QUERY_COLLISION query 'bug query'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_lifecycle_failure_control LIFECYCLE_HISTORY_REVERSED history 'bug history'; then
        control_failures=$((control_failures + 1))
    fi
    for field in summary url whiteboard severity priority; do
        if ! run_lifecycle_failure_control LIFECYCLE_STATE_NOOP_FIELD update 'bug update' "$field"; then
            control_failures=$((control_failures + 1))
        fi
        if ! run_lifecycle_failure_control LIFECYCLE_HISTORY_OMIT_FIELD history 'bug history' "$field"; then
            control_failures=$((control_failures + 1))
        fi
    done
    for target in bzr-view pybz-view; do
        if ! run_lifecycle_failure_control LIFECYCLE_WRONG_ID_TARGET view 'bug view' "$target"; then
            control_failures=$((control_failures + 1))
        fi
    done
    if ! run_lifecycle_failure_control LIFECYCLE_WRONG_ID_TARGET history 'bug history' history; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_lifecycle_failure_control LIFECYCLE_HISTORY_EXTRA history 'bug history'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_partial_stale_gap_control LIFECYCLE_GENERIC_CREATE_NOOP 671 arbitrary-fields \
        'generic arbitrary fields'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_partial_stale_gap_control LIFECYCLE_MINOR_UPDATE_OMITTED 672 update-options \
        'comment tags and minor update'; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_noop_stale_gap_control; then
        control_failures=$((control_failures + 1))
    fi

    PYBZ_LIFECYCLE_MISMATCH=1
    PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
    SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 LIFECYCLE_UPDATED=0
    source "$phase" >>"$fixture_output"
    _render_test_result >>"$fixture_output"
    if test_summary >>"$fixture_output"; then
        printf 'lifecycle mismatch unexpectedly passed\n' >&2
        return 1
    fi
    if ! grep -Fq 'compare/01-bug-lifecycle/view' "$fixture_output"; then
        printf 'lifecycle mismatch did not name the view capability\n' >&2
        return 1
    fi
    unset PYBZ_LIFECYCLE_MISMATCH

    LIFECYCLE_STALE_GAPS=1
    LIFECYCLE_PYBZ_TAGGED=0 LIFECYCLE_GENERIC_CREATE_COUNT=0 LIFECYCLE_GENERIC_UPDATED=0
    PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
    SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 LIFECYCLE_UPDATED=0
    : >"$LIFECYCLE_BZR_ARGS"
    source "$phase" >>"$fixture_output"
    _render_test_result >>"$fixture_output"
    if test_summary >>"$fixture_output"; then
        printf 'stale gap controls unexpectedly passed\n' >&2
        return 1
    fi
    for issue in 670 671 672 679 680; do
        if ! grep -Fq "#${issue} appears resolved" "$fixture_output"; then
            printf 'stale gap control did not name #%s\n' "$issue" >&2
            return 1
        fi
    done
    assert_equals 5 "$FAIL_COUNT" "stale gap fail count"
    if ! jq -e '.minor_update == true' \
        "$COMPARE_EXCHANGE_DIR/update-options-bzr.request.json" >/dev/null; then
        printf 'stale update-options control omitted minor_update request payload\n' >&2
        return 1
    fi
    unset LIFECYCLE_STALE_GAPS
    if [[ $control_failures -ne 0 ]]; then
        return 1
    fi
)

run_parity_report_fixture() {
    local report="$PYBZ_DIR/../../../docs/dev/python-bugzilla-parity.md"
    local row
    # shellcheck disable=SC2016 # Markdown code spans are literal fixture data.
    local -a rows=(
        '| Bug create and first description | `bzr bug create`, `bzr comment list`, `bzr bug view` | parity | `compare/01-bug-lifecycle/create` |'
        '| Bug query | `bzr bug list` | parity | `compare/01-bug-lifecycle/query` |'
        '| Bug update | `bzr bug update` | parity | `compare/01-bug-lifecycle/update` |'
        '| Bug view | `bzr bug view` | parity | `compare/01-bug-lifecycle/view` |'
        '| Bug history | `bzr bug history` | parity | `compare/01-bug-lifecycle/history` |'
        '| Server saved search | `bzr bug search --saved-search` | expected gap (#670) | `compare/01-bug-lifecycle/saved-search` |'
        '| Generic arbitrary fields | `bzr bug create/update --field` | expected gap (#671) | `compare/01-bug-lifecycle/arbitrary-fields` |'
        '| Comment tags and minor update | `bzr bug update --comment-tag --minor-update` | expected gap (#672) | `compare/01-bug-lifecycle/update-options` |'
        '| Whiteboard match types | `bzr bug list --status-whiteboard-type` | expected gap (#679) | `compare/01-bug-lifecycle/query-match-types` |'
        '| Personal bug tags | `bzr bug tag`, `bzr bug list --tag` | expected gap (#680) | `compare/01-bug-lifecycle/bug-tags` |'
    )

    for row in "${rows[@]}"; do
        if [[ $(grep -Fxc "$row" "$report") -ne 1 ]]; then
            printf 'missing or duplicate parity report row: %s\n' "$row" >&2
            return 1
        fi
    done
}

run_sidecar_stop_failure_fixture() (
    local error_output
    error_output=$(mktemp)
    trap 'rm -f "$error_output"' EXIT
    BZ_VERSION=bz50
    PYBZ_RUNTIME=fake_runtime

    # Invoked indirectly through pybz_sidecar_stop's runtime argument.
    # shellcheck disable=SC2317,SC2329
    fake_runtime() {
        if [[ $1 == container && $2 == inspect ]]; then
            return 0
        fi
        if [[ $1 == rm && $2 == -f ]]; then
            return 1
        fi
        return 2
    }

    if pybz_sidecar_stop fake_runtime 2>"$error_output"; then
        printf 'sidecar removal failure was ignored\n' >&2
        return 1
    fi
    if ! grep -Fq 'pybz_sidecar_stop: could not remove sidecar:' "$error_output"; then
        printf 'sidecar removal failure omitted its diagnostic\n' >&2
        return 1
    fi
    assert_equals fake_runtime "$PYBZ_RUNTIME" "failed sidecar ownership"
)

run_adapter_staging_cleanup_fixture() (
    local fixture_root
    local residue
    local status
    local executable
    fixture_root=$(mktemp -d)
    residue="$fixture_root/compare-config"
    executable=$(command -v bash)
    trap 'rm -rf "$fixture_root"' EXIT

    mktemp() {
        local path
        case "$*" in
        '-d /tmp/bzr-compare-config.XXXXXX') path="$residue" ;;
        /tmp/bzr-func-stdout.XXXXXX) path="$fixture_root/stdout" ;;
        /tmp/bzr-func-stdout-raw.XXXXXX) path="$fixture_root/stdout-raw" ;;
        /tmp/bzr-func-stderr.XXXXXX) path="$fixture_root/stderr" ;;
        *) command mktemp "$@"; return ;;
        esac
        if [[ ${1:-} == -d ]]; then
            mkdir -p "$path"
        else
            : >"$path"
        fi
        printf '%s\n' "$path"
    }
    cp() {
        if [[ ${2:-} == "$residue/compare/bug-lifecycle.py" ]]; then
            return 1
        fi
        command cp "$@"
    }

    set +e
    (
        BZR_COMPARE_BIN="$executable" BZR_FUNC_PORT=1
        # shellcheck source=tests/functional/run-compare.sh
        source "$PYBZ_DIR/../run-compare.sh"
    )
    status=$?
    set -e
    assert_equals 1 "$status" "adapter staging failure status"
    if [[ -e $residue ]]; then
        printf 'adapter staging failure left exchange residue: %s\n' "$residue" >&2
        return 1
    fi
)

write_fake_bugzilla_module() {
    local fixture_dir="$1"

    mkdir -p "$fixture_dir/bugzilla"
    cat >"$fixture_dir/bugzilla/__init__.py" <<'PY'
class _FixtureAutoBackend:
    pass


class _FixtureRESTBackend:
    def __init__(self):
        self.comment_tags = []

    def _put(self, path, payload):
        if path != "/bug/comment/350/tags" or payload != {"add": ["probe"]}:
            raise RuntimeError("unexpected comment-tag request")
        self.comment_tags = payload["add"]
        raise ValueError("array response")


class _FixtureXMLRPCBackend:
    pass


class _FixtureBug:
    def __init__(self, data):
        self._data = data

    def get_raw_data(self):
        return self._data


class Bugzilla:
    def __init__(
        self,
        url,
        api_key=None,
        use_creds=True,
        force_rest=False,
        force_xmlrpc=False,
    ):
        if (
            url != "http://127.0.0.1"
            or api_key != "fixture-secret"
            or use_creds
            or (force_rest and force_xmlrpc)
        ):
            raise RuntimeError("unexpected constructor arguments")
        self._backend = (
            _FixtureXMLRPCBackend()
            if force_xmlrpc
            else _FixtureRESTBackend()
            if force_rest
            else _FixtureAutoBackend()
        )

    def build_createbug(self, **params):
        return {"builder": "create", **params}

    def createbug(self, params):
        return _FixtureBug({"id": 101, "request": params})

    def build_query(self, **params):
        return {"builder": "query", **params}

    def query(self, query):
        return [_FixtureBug({"id": 201, "request": query})]

    def build_update(self, **params):
        if "comment" in params:
            params["comment"] = {"comment": params["comment"]}
        return {"builder": "update", **params}

    def update_bugs(self, ids, update):
        return {"ids": ids, "update": update}

    def get_comments(self, ids):
        return {
            "bugs": {
                str(ids[0]): {
                    "comments": [
                        {
                            "id": 350,
                            "text": "tagged comment",
                            "tags": getattr(self._backend, "comment_tags", []),
                        }
                    ]
                }
            }
        }

    def getbug(self, bug_id):
        data = {"id": bug_id, "source": "view"}
        if bug_id == 37:
            from xmlrpc.client import DateTime

            data["last_change_time"] = DateTime("20260101T00:00:00")
        return _FixtureBug(data)

    def bugs_history_raw(self, bug_ids):
        return {
            "bugs": [
                {
                    "id": bug_ids[0],
                    "history": [
                        {
                            "when": "fixture",
                            "changes": [
                                {
                                    "field_name": "summary",
                                    "removed": "old",
                                    "added": "new",
                                }
                            ],
                        }
                    ],
                }
            ]
        }

    def update_tags(self, ids, tags_add=None, tags_remove=None):
        return {"ids": ids, "add": tags_add, "remove": tags_remove}
PY
}

assert_adapter_case() {
    local runtime="$1"
    local sidecar="$2"
    local config_dir="$3"
    local name="$4"
    local operation="$5"
    local request="$6"
    local expected="$7"
    local input="$config_dir/${name}.input.json"
    local output="$config_dir/${name}.output.json"
    local actual

    printf '%s\n' "$request" >"$input"
    chmod 600 "$input"
    "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/bug-lifecycle.py "$operation" "/work/${name}.input.json" \
        "/work/${name}.output.json"
    if ! jq -e '.transport | type == "string" and length > 0' "$output" >/dev/null; then
        printf 'adapter case %s omitted transport\n' "$name" >&2
        return 1
    fi
    actual=$(jq -cS . "$output")
    assert_equals "$expected" "$actual" "adapter $name result"
}

run_adapter_fixture() {
    local runtime="$1"
    local sidecar="$2"
    local config_dir="$3"
    local adapter="$PYBZ_DIR/../compare/bug-lifecycle.py"
    local error_output="$config_dir/adapter-error.stderr"
    local invalid_input="$config_dir/invalid-id.input.json"
    local invalid_status

    if [[ ! -r $adapter ]]; then
        printf 'python-bugzilla lifecycle adapter is missing: %s\n' "$adapter" >&2
        return 1
    fi
    cp "$adapter" "$config_dir/bug-lifecycle.py"
    chmod 600 "$config_dir/bug-lifecycle.py"
    write_fake_bugzilla_module "$config_dir/adapter-fixture"

    "$runtime" exec "$sidecar" python -m py_compile /work/bug-lifecycle.py
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" create create \
        '{"api_key":"fixture-secret","params":{"product":"Widget","summary":"create"}}' \
        '{"result":{"id":101,"request":{"builder":"create","product":"Widget","summary":"create"}},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" query query \
        '{"api_key":"fixture-secret","params":{"short_desc":"needle"}}' \
        '{"result":[{"id":201,"request":{"builder":"query","short_desc":"needle"}}],"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" update update \
        '{"api_key":"fixture-secret","bug_id":31,"params":{"summary":"updated"}}' \
        '{"result":{"ids":[31],"update":{"builder":"update","summary":"updated"}},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" view view \
        '{"api_key":"fixture-secret","bug_id":32}' \
        '{"result":{"id":32,"source":"view"},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" view-xmlrpc-date view \
        '{"api_key":"fixture-secret","bug_id":37}' \
        '{"result":{"id":37,"last_change_time":"20260101T00:00:00","source":"view"},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" history history \
        '{"api_key":"fixture-secret","bug_id":33}' \
        '{"result":{"bugs":[{"history":[{"changes":[{"added":"new","field_name":"summary","removed":"old"}],"when":"fixture"}],"id":33}]},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" saved-search saved_search \
        '{"api_key":"fixture-secret","name":"owned-search"}' \
        '{"result":[{"id":201,"request":{"builder":"query","savedsearch":"owned-search"}}],"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" generic-create generic_fields \
        '{"api_key":"fixture-secret","action":"create","params":{"product":"Widget","summary":"generic"},"fields":{"cf_probe":"initial"}}' \
        '{"result":{"id":101,"request":{"builder":"create","cf_probe":"initial","product":"Widget","summary":"generic"}},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" generic-update generic_fields \
        '{"api_key":"fixture-secret","action":"update","bug_id":34,"params":{"summary":"generic-updated"},"fields":{"cf_probe":"changed"}}' \
        '{"result":{"ids":[34],"update":{"builder":"update","cf_probe":"changed","summary":"generic-updated"}},"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" update-options update_options \
        '{"api_key":"fixture-secret","bug_id":35,"comment":"tagged comment","comment_tags":["probe"],"minor_update":true}' \
        '{"result":{"ids":[35],"update":{"builder":"update","comment":{"comment":"tagged comment"},"minor_update":true}},"transport":"_FixtureRESTBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" match-type match_type \
        '{"api_key":"fixture-secret","value":"needle","match_type":"equals"}' \
        '{"result":[{"id":201,"request":{"builder":"query","status_whiteboard":"needle","status_whiteboard_type":"equals"}}],"transport":"_FixtureAutoBackend"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" bug-tags bug_tags \
        '{"api_key":"fixture-secret","bug_id":36,"tag":"probe"}' \
        '{"result":{"bugs":[{"id":201,"request":{"builder":"query","tags":["probe"]}}],"update":{"add":["probe"],"ids":[36],"remove":null}},"transport":"_FixtureXMLRPCBackend"}'

    printf '%s\n' '{"api_key":"fixture-secret","bug_id":0}' >"$invalid_input"
    chmod 600 "$invalid_input"
    set +e
    "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/bug-lifecycle.py view /work/invalid-id.input.json \
        /work/invalid-id.output.json 2>"$error_output"
    invalid_status=$?
    set -e
    assert_equals 1 "$invalid_status" "invalid adapter ID status"
    if grep -Fq fixture-secret "$error_output"; then
        printf 'adapter error leaked the API key\n' >&2
        return 1
    fi
    if ! grep -Fq '/work/invalid-id.input.json' "$error_output"; then
        printf 'adapter error omitted the input path\n' >&2
        return 1
    fi
    if "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/bug-lifecycle.py unsupported /work/invalid-id.input.json \
        /work/unsupported.output.json 2>"$error_output"; then
        printf 'adapter accepted an unsupported operation\n' >&2
        return 1
    fi
    if "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/bug-lifecycle.py view /work/invalid-id.input.json \
        2>"$error_output"; then
        printf 'adapter accepted an incomplete argument list\n' >&2
        return 1
    fi
}

cleanup_container_fixture() {
    local runtime="$1"
    local donor="$2"
    local config_dir="$3"

    pybz_sidecar_stop "$runtime"
    if "$runtime" container inspect "$donor" >/dev/null 2>&1; then
        "$runtime" rm -f "$donor" >/dev/null
    fi
    rm -rf "$config_dir"
    return 0
}

run_container_fixture() {
    local runtime
    local checkout_id
    local fixture_image
    local donor
    local config_dir
    local package_version
    local cli_version
    local sidecar
    local sidecar_id
    local collision_error
    local collision_status
    local replacement_id

    runtime=$(container_runtime) || {
        printf 'no container runtime available\n' >&2
        return 1
    }
    checkout_id=$(bugzilla_checkout_id)
    fixture_image="localhost/bzr-pybz-fixture-${checkout_id}:3.3.0"
    donor="bzr-pybz-fixture-${checkout_id}"
    config_dir=$(mktemp -d)
    export FUNC_CONFIG_DIR="$config_dir"
    BZ_VERSION="bz50"
    trap 'cleanup_container_fixture "$runtime" "$donor" "$config_dir"' RETURN

    "$runtime" build -t "$fixture_image" -f "$PYBZ_DIR/Containerfile" "$PYBZ_DIR"
    package_version=$("$runtime" run --rm "$fixture_image" python -c \
        'from importlib.metadata import version; print(version("python-bugzilla"))')
    assert_equals 3.3.0 "$package_version" "python-bugzilla version"
    cli_version=$("$runtime" run --rm "$fixture_image" bugzilla --version)
    if [[ $cli_version != *3.3.0* ]]; then
        printf 'bugzilla CLI did not report version 3.3.0\n' >&2
        return 1
    fi

    if "$runtime" container inspect "$donor" >/dev/null 2>&1; then
        "$runtime" rm -f "$donor" >/dev/null
    fi
    "$runtime" run -d --name "$donor" "$fixture_image" >/dev/null
    pybz_sidecar_start "$runtime" "$donor"

    sidecar=$(pybz_sidecar_name)
    sidecar_id=$("$runtime" container inspect --format '{{.Id}}' "$sidecar")
    collision_error="$config_dir/running-sidecar.stderr"
    PYBZ_RUNTIME=''
    set +e
    pybz_sidecar_start "$runtime" "$donor" 2>"$collision_error"
    collision_status=$?
    set -e
    assert_equals 1 "$collision_status" "running sidecar collision status"
    assert_equals "$sidecar_id" \
        "$("$runtime" container inspect --format '{{.Id}}' "$sidecar")" \
        "running sidecar identity"
    assert_equals '' "$PYBZ_RUNTIME" "running sidecar ownership"
    if ! grep -Fq "sidecar is already running: $sidecar" "$collision_error"; then
        printf 'running sidecar collision omitted its actionable diagnostic\n' >&2
        return 1
    fi
    "$runtime" stop "$sidecar" >/dev/null
    pybz_sidecar_start "$runtime" "$donor"
    replacement_id=$("$runtime" container inspect --format '{{.Id}}' "$sidecar")
    if [[ $replacement_id == "$sidecar_id" ]]; then
        printf 'stopped sidecar was not replaced\n' >&2
        return 1
    fi
    assert_equals true \
        "$("$runtime" container inspect --format '{{.State.Running}}' "$sidecar")" \
        "replacement sidecar running state"

    run_pybz --version
    assert_success
    run_pybz --definitely-invalid-option
    assert_failure

    "$runtime" exec "$sidecar" sh -c "printf '%s' exchange-proof > /work/proof"
    assert_equals exchange-proof "$(<"$config_dir/proof")" "bind-mount bytes"
    run_adapter_fixture "$runtime" "$sidecar" "$config_dir"
    return 0
}

run_expected_gap_fixture
run_summary_fixture
run_product_normalization_fixture
run_sidecar_wrapper_fixture
run_lifecycle_phase_fixture
run_parity_report_fixture
run_sidecar_stop_failure_fixture
run_adapter_staging_cleanup_fixture
run_container_fixture
