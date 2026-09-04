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
        'python /work/compare/python-bugzilla-adapter.py view /work/compare/in.json /work/compare/out.json' \
        "$(<"$invoked")" "fixed adapter command"
    assert_equals $'{\n  "fixed": true\n}' "$(<"$BZR_STDOUT")" "adapter projected stdout"
    assert_equals '{"schema_version":"3.0.0","data":{"fixed":true}}' \
        "$(<"$BZR_STDOUT_RAW")" "adapter raw stdout"
    assert_equals 'fixed stderr' "$(<"$BZR_STDERR")" "adapter stderr"
    assert_equals 19 "$BZR_EXIT" "adapter exit"
)

run_transport_observation_fixture() (
    if ! declare -F observe_bzr_transport >/dev/null; then
        printf 'observe_bzr_transport is not defined\n' >&2
        return 1
    fi

    printf 'DEBUG bzr::client::transport: API response\n' >"$BZR_STDERR"
    observe_bzr_transport
    assert_equals REST "$BZR_TRANSPORT" "single REST observation"

    printf '%s\n' \
        'DEBUG bzr::client::transport: API response' \
        'DEBUG bzr::client::transport: API response' >"$BZR_STDERR"
    observe_bzr_transport
    assert_equals REST "$BZR_TRANSPORT" "repeated REST observations"

    printf 'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call\n' >"$BZR_STDERR"
    observe_bzr_transport
    assert_equals XMLRPC "$BZR_TRANSPORT" "single XML-RPC observation"

    printf 'INFO bzr::client: overriding header auth for XML-RPC calls\n' >"$BZR_STDERR"
    if observe_bzr_transport; then
        printf 'non-boundary transport decoy was accepted\n' >&2
        return 1
    fi
    printf '%s\n' 'DEBUG bzr::client::transport: API response' \
        'INFO bzr::client: overriding header auth for XML-RPC calls' >"$BZR_STDERR"
    observe_bzr_transport
    assert_equals REST "$BZR_TRANSPORT" "REST observation with XML-RPC decoy"

    for decoy in 'bzr::client::transport: API response' \
        'bzr::xmlrpc::protocol::client: XML-RPC call'; do
        printf 'DEBUG unrelated::target: %s\n' "$decoy" >"$BZR_STDERR"
        if observe_bzr_transport; then
            printf 'wrong-target transport decoy was accepted: %s\n' "$decoy" >&2
            return 1
        fi
    done

    : >"$BZR_STDERR"
    if observe_bzr_transport; then
        printf 'missing transport observation was accepted\n' >&2
        return 1
    fi

    printf '%s\n' \
        'DEBUG bzr::client::transport: API response' \
        'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call' >"$BZR_STDERR"
    if observe_bzr_transport; then
        printf 'mixed transport observations were accepted\n' >&2
        return 1
    fi
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
    jq() {
        if [[ ${LIFECYCLE_DOWNSTREAM_ASSERTION_FAILED:-0} -eq 1 &&
            " $* " == *" --argjson expected "* &&
            " $* " == *" saved-search.bzr.stdout.json "* ]]; then
            return 2
        fi
        command jq "$@"
    }

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
    run_gap_ineligible_control() {
        local flag="$1" capability="$2" label="$3"

        reset_lifecycle_fixture
        LIFECYCLE_STALE_GAPS=1
        printf -v "$flag" 1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag" LIFECYCLE_STALE_GAPS
        if [[ $FAIL_COUNT -eq 0 || $GAP_COUNT -ne 0 ]] ||
            ! grep -Fq \
                "[compare/01-bug-lifecycle/${capability}] ${label} ... FAIL" \
                "$fixture_output"; then
            printf '%s ineligible control %s became a gap\n' "$capability" "$flag" >&2
            cat "$fixture_output" >&2
            return 1
        fi
        printf 'controlled ineligible: %s %s=1\n' "$capability" "$flag"
    }
    run_repeated_transport_control() {
        reset_lifecycle_fixture
        LIFECYCLE_REPEATED_REST_EVENTS=1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset LIFECYCLE_REPEATED_REST_EVENTS
        if [[ $FAIL_COUNT -ne 0 || $PASS_COUNT -ne 5 || $GAP_COUNT -ne 5 ]]; then
            printf 'repeated REST observations did not preserve lifecycle outcomes\n' >&2
            cat "$fixture_output" >&2
            return 1
        fi
    }
    run_observed_rest_gap_control() {
        reset_lifecycle_fixture
        LIFECYCLE_STALE_GAPS=1 LIFECYCLE_BUG_TAGS_OBSERVED_REST=1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset LIFECYCLE_STALE_GAPS LIFECYCLE_BUG_TAGS_OBSERVED_REST
        if [[ $GAP_COUNT -ne 1 ]] ||
            ! grep -Fq \
                '[compare/01-bug-lifecycle/bug-tags] personal bug tags ... GAP (#680)' \
                "$fixture_output"; then
            printf 'observed REST bug-tag operations did not remain gap #680\n' >&2
            cat "$fixture_output" >&2
            return 1
        fi
    }
    run_eligibility_reset_control() {
        reset_lifecycle_fixture
        LIFECYCLE_ELIGIBILITY_RESET_CONTROL=1
        : >"$fixture_output"
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset LIFECYCLE_ELIGIBILITY_RESET_CONTROL
        if ! grep -Fq \
            '[compare/01-bug-lifecycle/saved-search] server saved search ... GAP (#670)' \
            "$fixture_output" ||
            ! grep -Fq \
                '[compare/01-bug-lifecycle/arbitrary-fields] generic arbitrary fields ... FAIL' \
                "$fixture_output"; then
            printf 'gap eligibility leaked into the following probe\n' >&2
            cat "$fixture_output" >&2
            return 1
        fi
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
            cat "$fixture_output" >&2
            return 1
        fi
    }
    fixture_finish_bzr() {
        local exit_code="$1"
        local transport=REST

        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT="$exit_code"
        [[ $exit_code -eq 0 ]] || return 0
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == update-options-bzr-request ]]; then
            if [[ ${LIFECYCLE_NO_DISPATCH_EVENT:-0} -eq 1 ]]; then
                printf 'DEBUG bzr::client::transport: API response\n' >"$BZR_STDERR"
            fi
            return 0
        fi
        if [[ ${LIFECYCLE_MISSING_BZR_EVENTS:-0} -eq 1 &&
            ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search ]]; then
            return 0
        fi
        if [[ ${LIFECYCLE_MIXED_BZR_EVENTS:-0} -eq 1 &&
            ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search ]]; then
            printf '%s\n' 'DEBUG bzr::client::transport: API response' \
                'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call' >"$BZR_STDERR"
            return 0
        fi
        [[ $args == *" --api xmlrpc "* ]] && transport=XMLRPC
        if [[ ${LIFECYCLE_BUG_TAGS_OBSERVED_REST:-0} -eq 1 &&
            ${LIFECYCLE_BZR_CALL_NAME:-} == bug-tags-* ]]; then
            transport=REST
        fi
        if [[ $transport == REST ]]; then
            printf 'DEBUG bzr::client::transport: API response\n' >"$BZR_STDERR"
            if [[ ${LIFECYCLE_REPEATED_REST_EVENTS:-0} -eq 1 ]]; then
                printf 'DEBUG bzr::client::transport: API response\n' >>"$BZR_STDERR"
            fi
        else
            printf 'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call\n' >"$BZR_STDERR"
        fi
    }
    run_bzr() {
        local args=" $* " id=41 summary="$LIFECYCLE_BZR_SUMMARY" value diagnostic
        printf '%s\n' "$*" >>"$LIFECYCLE_BZR_ARGS"
        : >"$BZR_STDOUT"
        [[ $args == *" 42 "* ]] && id=42 && summary="$LIFECYCLE_PYBZ_SUMMARY"
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == view-bzr &&
            ${LIFECYCLE_WRONG_ID_TARGET:-} == bzr-view ]]; then id=99; fi
        if [[ ${LIFECYCLE_UPDATED:-0} -eq 1 ]]; then summary="$LIFECYCLE_UPDATED_SUMMARY"; fi
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search &&
            ( ${LIFECYCLE_WRONG_PARSER_DIAGNOSTIC:-0} -eq 1 ||
                ${LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE:-0} -eq 1 ) ]]; then
            diagnostic="error: unexpected argument '--saved-search' found"
            [[ ${LIFECYCLE_WRONG_PARSER_DIAGNOSTIC:-0} -eq 0 ]] ||
                diagnostic="error: unexpected argument '--different-option' found"
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
            printf '%s\n' "$diagnostic" >"$BZR_STDERR"
            BZR_EXIT=2
            [[ ${LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE:-0} -eq 0 ]] || BZR_EXIT=1
            return 0
        fi
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search &&
            ${LIFECYCLE_CONNECTION_FAILURE:-0} -eq 1 ]]; then
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
            : >"$BZR_STDERR"
            BZR_EXIT=4
            return 0
        fi
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search &&
            ${LIFECYCLE_SERVER_COMMAND_ERROR:-0} -eq 1 ]]; then
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
            printf 'server rejected fixture operation\n' >"$BZR_STDERR"
            BZR_EXIT=5
            return 0
        fi
        if [[ ${LIFECYCLE_ELIGIBILITY_RESET_CONTROL:-0} -eq 1 &&
            ${LIFECYCLE_BZR_CALL_NAME:-} == arbitrary-fields-create ]]; then
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
            : >"$BZR_STDERR"
            BZR_EXIT=4
            return 0
        fi
        if [[ ${LIFECYCLE_NOOP_STALE_GAPS:-0} -eq 1 &&
            ( $args == *" --comment-tag "* || $args == *" bug tag "* ) ]]; then
            printf '{}\n' >"$BZR_STDOUT"
            fixture_finish_bzr 0
            return 0
        fi
        if [[ ${LIFECYCLE_NOOP_STALE_GAPS:-0} -eq 1 && $args == *" bug list "* &&
            $args == *" --tag "* ]]; then
            if [[ $args == *" --tag $LIFECYCLE_BUG_TAG "* ]]; then
                printf '[{"id":42}]\n' >"$BZR_STDOUT"
            else
                printf '[]\n' >"$BZR_STDOUT"
            fi
            fixture_finish_bzr 0
            return 0
        fi
        if [[ ${LIFECYCLE_STALE_GAPS:-0} -eq 1 && $args == *" bug list "* &&
            $args == *" --tag "* ]]; then
            printf '[{"id":42}]\n' >"$BZR_STDOUT"
            fixture_finish_bzr 0
            return 0
        fi
        if [[ ${LIFECYCLE_STALE_GAPS:-0} -ne 1 &&
            ( $args == *" --saved-search "* || $args == *" --field "* ||
                $args == *" --comment-tag "* || $args == *" --status-whiteboard-type "* ||
                $args == *" bug tag "* || $args == *" --tag "* ) ]]; then
            case "$args" in
            *" --saved-search "*) diagnostic="error: unexpected argument '--saved-search' found" ;;
            *" --field "*) diagnostic="error: unexpected argument '--field' found" ;;
            *" --comment-tag "*) diagnostic="error: unexpected argument '--comment-tag' found" ;;
            *" --status-whiteboard-type "*)
                diagnostic="error: unexpected argument '--status-whiteboard-type' found"
                ;;
            *" bug tag "*) diagnostic="error: unrecognized subcommand 'tag'" ;;
            *) diagnostic="error: unexpected argument '--tag' found" ;;
            esac
            if [[ ${LIFECYCLE_WRONG_PARSER_DIAGNOSTIC:-0} -eq 1 ]]; then
                diagnostic="error: unexpected argument '--different-option' found"
            fi
            cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
            printf '%s\n' "$diagnostic" >"$BZR_STDERR"
            BZR_EXIT=2
            [[ ${LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE:-0} -eq 0 ]] || BZR_EXIT=1
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
                if [[ ${LIFECYCLE_MALFORMED_BZR_RESULT:-0} -eq 1 &&
                    ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search ]]; then
                    printf '{invalid\n' >"$BZR_STDOUT"
                fi
                if [[ ${LIFECYCLE_INVALID_NO_DISPATCH_RESULT:-0} -eq 1 &&
                    ${LIFECYCLE_BZR_CALL_NAME:-} == update-options-bzr-request ]]; then
                    printf '{invalid\n' >"$BZR_STDOUT"
                fi
                if [[ ${LIFECYCLE_INVALID_NO_DISPATCH_SHAPE:-0} -eq 1 &&
                    ${LIFECYCLE_BZR_CALL_NAME:-} == update-options-bzr-request ]]; then
                    printf 'true\n' >"$BZR_STDOUT"
                fi
                fixture_finish_bzr 0
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
                : >"$BZR_STDOUT"
                fixture_finish_bzr 2
                return 0
            fi
            return 2
            ;;
        esac
        fixture_finish_bzr 0
    }
    fake_lifecycle_runtime() {
        local operation="$5" output="$COMPARE_EXCHANGE_DIR/${7##*/}" result transport=XMLRPC
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
        update_options) LIFECYCLE_PYBZ_TAGGED=1; result='{}'; transport=REST ;;
        bug_tags) result='{"bugs":[{"id":42}],"update":{}}'; transport=XMLRPC ;;
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
        if [[ $operation == create && ${LIFECYCLE_UNKNOWN_PYBZ_TRANSPORT:-0} -eq 1 ]]; then
            transport=RESTFUL
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
    if ! run_lifecycle_failure_control LIFECYCLE_UNKNOWN_PYBZ_TRANSPORT create \
        'bug create and first description'; then
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
    for control in \
        LIFECYCLE_MISSING_BZR_EVENTS \
        LIFECYCLE_MIXED_BZR_EVENTS \
        LIFECYCLE_CONNECTION_FAILURE \
        LIFECYCLE_SERVER_COMMAND_ERROR \
        LIFECYCLE_WRONG_PARSER_DIAGNOSTIC \
        LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE \
        LIFECYCLE_MALFORMED_BZR_RESULT \
        LIFECYCLE_DOWNSTREAM_ASSERTION_FAILED; do
        if ! run_gap_ineligible_control "$control" saved-search 'server saved search'; then
            control_failures=$((control_failures + 1))
        fi
    done
    for control in LIFECYCLE_NO_DISPATCH_EVENT LIFECYCLE_INVALID_NO_DISPATCH_RESULT \
        LIFECYCLE_INVALID_NO_DISPATCH_SHAPE; do
        if ! run_gap_ineligible_control "$control" update-options \
            'comment tags and minor update'; then
            control_failures=$((control_failures + 1))
        fi
    done
    if ! run_repeated_transport_control; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_observed_rest_gap_control; then
        control_failures=$((control_failures + 1))
    fi
    if ! run_eligibility_reset_control; then
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
        '| Public comments | `bzr comment add`, `bzr comment list` | parity | `compare/02-comments/public-comments` |'
        '| Private comments over REST | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-rest` |'
        '| Private comments over XML-RPC | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-xmlrpc` |'
        '| Attachment upload metadata and comment | `bzr attachment upload`, `bzr attachment list`, `bzr comment list` | parity | `compare/03-attachments/upload-metadata-comment` |'
        '| Attachment download content | `bzr attachment download` | parity | `compare/03-attachments/download-content` |'
        '| Attachment flags | `bzr attachment update --flag` | parity | `compare/03-attachments/attachment-flags` |'
        '| Private attachments over REST | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-rest` |'
        '| Private attachments over XML-RPC | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-xmlrpc` |'
        '| Multi-bug attachment upload | `bzr attachment upload` | expected gap (#674) | `compare/03-attachments/multi-bug-upload` |'
        '| Ignore obsolete attachments | `bzr attachment download --bug --ignore-obsolete` | expected gap (#674) | `compare/03-attachments/ignore-obsolete` |'
        '| User create, get, and search | `bzr user create`, `bzr user search` | parity | `compare/04-users-groups/user-create-get-search` |'
        '| Group get and list | `bzr group view` | parity | `compare/04-users-groups/group-get-and-list` |'
        '| Membership add and remove | `bzr group add-user/remove-user`, `bzr user search` | parity | `compare/04-users-groups/membership-add-remove` |'
        '| Product catalogues | `bzr product list --type` | parity | `compare/05-products-components/product-catalogues` |'
        '| Component create | `bzr component create`, `bzr component view` | parity | `compare/05-products-components/component-create` |'
        '| Red Hat component update | `bzr component update` | expected gap (#675) | `compare/05-products-components/component-update-redhat` |'
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
        if [[ ${2:-} == "$residue/compare/python-bugzilla-adapter.py" ]]; then
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
class _BackendREST:
    def __init__(self):
        self.comment_tags = []

    def _put(self, path, payload):
        if path != "/bug/comment/350/tags" or payload != {"add": ["probe"]}:
            raise RuntimeError("unexpected comment-tag request")
        self.comment_tags = payload["add"]
        raise ValueError("array response")


class _BackendXMLRPC:
    pass


class _UnknownBackend:
    pass


class _FixtureBug:
    def __init__(self, data):
        self._data = data

    def get_raw_data(self):
        return self._data


class _FixtureUser:
    def __init__(self, email):
        self.userid = 601
        self.email = email
        self.name = email
        self.real_name = "Fixture User"
        self.can_login = True
        self.groupnames = ["editbugs"]


class _FixtureGroup:
    groupid = 701
    name = "editbugs"
    description = "Can edit bugs"
    is_active = True
    member_emails = ["fixture-user@test.invalid"]


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
            _UnknownBackend()
            if __import__("os").environ.get("FIXTURE_UNKNOWN_BACKEND") == "1"
            else _BackendXMLRPC()
            if force_xmlrpc
            else _BackendREST()
            if force_rest
            else _BackendXMLRPC()
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
            comment = {"comment": params.pop("comment")}
            if params.pop("comment_private", False):
                comment["is_private"] = True
            params["comment"] = comment
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

    def attachfile(self, ids, source, summary, **kwargs):
        with open(source, "rb") as attachment:
            data = attachment.read().decode("utf-8")
        return 451 if ids == [41] and data == "fixture attachment\n" else 0

    def get_attachments(
        self, ids, attachment_ids, include_fields=None, exclude_fields=None
    ):
        if include_fields is not None:
            raise RuntimeError("unexpected attachment include_fields")
        if exclude_fields is None and ids:
            current_name = "../outside.txt" if ids == [42] else "current.txt"
            return {
                "bugs": {
                    str(ids[0]): [
                        {"id": 451, "file_name": "obsolete.txt", "is_obsolete": 1},
                        {"id": 452, "file_name": current_name, "is_obsolete": 0},
                    ]
                }
            }
        if exclude_fields != ["data"]:
            raise RuntimeError("attachment metadata must exclude data")
        return {
            "bugs": ids,
            "attachment_ids": attachment_ids,
            "attachments": {"451": {"id": 451}},
        }

    def openattachment(self, attachment_id):
        from io import BytesIO

        if attachment_id != 451:
            raise RuntimeError("fixture upstream attachment detail")
        return BytesIO(b"fixture attachment\n")

    def openattachment_data(self, attachment):
        from io import BytesIO

        stream = BytesIO(b"fixture attachment\n")
        stream.name = attachment["file_name"]
        return stream

    def updateattachmentflags(self, bug_id, attachment_id, flag_name, **kwargs):
        return {
            "bug_id": bug_id,
            "attachment_id": attachment_id,
            "flag_name": flag_name,
            **kwargs,
        }

    def createuser(self, email, name="", password=""):
        if not password:
            raise RuntimeError("fixture upstream password detail")
        return _FixtureUser(email)

    def getuser(self, email):
        return _FixtureUser(email)

    def searchusers(self, pattern):
        return [_FixtureUser(pattern)]

    def updateperms(self, user, action, groups):
        return {"user": user, "action": action, "groups": groups}

    def getgroup(self, name, membership=False):
        return _FixtureGroup()

    def getgroups(self, names, membership=False):
        return [_FixtureGroup()]

    def product_get(self, ptype=None, names=None):
        return [{"id": 801, "name": ptype or names[0]}]

    def addcomponent(self, data):
        return {"id": 901, "request": data}
PY
    cat >"$fixture_dir/bugzilla/_cli.py" <<'PY'
import os


def open_without_clobber(name, mode):
    stem, extension = os.path.splitext(name)
    candidate = name
    suffix = 0
    while True:
        try:
            descriptor = os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o666)
            return os.fdopen(descriptor, mode)
        except FileExistsError:
            suffix += 1
            candidate = f"{stem}-{suffix}{extension}"


def _do_get_attach(client, options):
    attachments = client.get_attachments(options.getall, None)["bugs"]
    for values in attachments.values():
        for attachment in values:
            if options.ignore_obsolete and attachment.get("is_obsolete") == 1:
                continue
            source = client.openattachment_data(attachment)
            with open_without_clobber(source.name, "wb") as destination:
                destination.write(source.read())
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
    local input="$config_dir/compare/${name}.input.json"
    local output="$config_dir/compare/${name}.output.json"
    local actual

    printf '%s\n' "$request" >"$input"
    chmod 600 "$input"
    "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/compare/python-bugzilla-adapter.py "$operation" \
        "/work/compare/${name}.input.json" "/work/compare/${name}.output.json"
    actual=$(jq -cS . "$output")
    assert_equals "$expected" "$actual" "adapter $name result"
}

assert_adapter_rejection() {
    local runtime="$1"
    local sidecar="$2"
    local config_dir="$3"
    local name="$4"
    local operation="$5"
    local request="$6"
    local diagnostic="$7"
    local input="$config_dir/compare/${name}.input.json"
    local output="$config_dir/compare/${name}.output.json"
    local error_output="$config_dir/compare/${name}.stderr"
    local status

    printf '%s\n' "$request" >"$input"
    chmod 600 "$input"
    set +e
    "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/compare/python-bugzilla-adapter.py "$operation" \
        "/work/compare/${name}.input.json" "/work/compare/${name}.output.json" \
        2>"$error_output"
    status=$?
    set -e
    assert_equals 1 "$status" "adapter $name rejection status"
    if ! grep -Fq "$diagnostic" "$error_output"; then
        printf 'adapter rejection %s omitted diagnostic: %s\n' "$name" "$diagnostic" >&2
        return 1
    fi
    if grep -Eq 'fixture-secret|/work/compare|fixture upstream' "$error_output"; then
        printf 'adapter rejection %s leaked private failure detail\n' "$name" >&2
        return 1
    fi
    if [[ -e $output ]]; then
        printf 'adapter rejection %s created an output file\n' "$name" >&2
        return 1
    fi
}

run_adapter_fixture() {
    local runtime="$1"
    local sidecar="$2"
    local config_dir="$3"
    local adapter="$PYBZ_DIR/../compare/python-bugzilla-adapter.py"
    local error_output="$config_dir/adapter-error.stderr"
    local invalid_input="$config_dir/compare/invalid-id.input.json"
    local local_input="$config_dir/compare/component-update-local.input.json"
    local local_output="$config_dir/compare/component-update-local.output.json"
    local invalid_status

    if [[ ! -r $adapter ]]; then
        printf 'python-bugzilla comparison adapter is missing: %s\n' "$adapter" >&2
        return 1
    fi
    mkdir -p "$config_dir/compare"
    cp "$adapter" "$config_dir/compare/python-bugzilla-adapter.py"
    chmod 600 "$config_dir/compare/python-bugzilla-adapter.py"
    printf 'fixture attachment\n' >"$config_dir/compare/attachment.txt"
    chmod 600 "$config_dir/compare/attachment.txt"
    write_fake_bugzilla_module "$config_dir/adapter-fixture"

    "$runtime" exec "$sidecar" python -m py_compile \
        /work/compare/python-bugzilla-adapter.py
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" create create \
        '{"api_key":"fixture-secret","params":{"product":"Widget","summary":"create"}}' \
        '{"result":{"id":101,"request":{"builder":"create","product":"Widget","summary":"create"}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" query query \
        '{"api_key":"fixture-secret","params":{"short_desc":"needle"}}' \
        '{"result":[{"id":201,"request":{"builder":"query","short_desc":"needle"}}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" update update \
        '{"api_key":"fixture-secret","bug_id":31,"params":{"summary":"updated"}}' \
        '{"result":{"ids":[31],"update":{"builder":"update","summary":"updated"}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" view view \
        '{"api_key":"fixture-secret","bug_id":32}' \
        '{"result":{"id":32,"source":"view"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" view-xmlrpc-date view \
        '{"api_key":"fixture-secret","bug_id":37}' \
        '{"result":{"id":37,"last_change_time":"20260101T00:00:00","source":"view"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" history history \
        '{"api_key":"fixture-secret","bug_id":33}' \
        '{"result":{"bugs":[{"history":[{"changes":[{"added":"new","field_name":"summary","removed":"old"}],"when":"fixture"}],"id":33}]},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" saved-search saved_search \
        '{"api_key":"fixture-secret","name":"owned-search"}' \
        '{"result":[{"id":201,"request":{"builder":"query","savedsearch":"owned-search"}}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" generic-create generic_fields \
        '{"api_key":"fixture-secret","action":"create","params":{"product":"Widget","summary":"generic"},"fields":{"cf_probe":"initial"}}' \
        '{"result":{"id":101,"request":{"builder":"create","cf_probe":"initial","product":"Widget","summary":"generic"}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" generic-update generic_fields \
        '{"api_key":"fixture-secret","action":"update","bug_id":34,"params":{"summary":"generic-updated"},"fields":{"cf_probe":"changed"}}' \
        '{"result":{"ids":[34],"update":{"builder":"update","cf_probe":"changed","summary":"generic-updated"}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" update-options update_options \
        '{"api_key":"fixture-secret","bug_id":35,"comment":"tagged comment","comment_tags":["probe"],"minor_update":true}' \
        '{"result":{"ids":[35],"update":{"builder":"update","comment":{"comment":"tagged comment"},"minor_update":true}},"transport":"REST"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" match-type match_type \
        '{"api_key":"fixture-secret","value":"needle","match_type":"equals"}' \
        '{"result":[{"id":201,"request":{"builder":"query","status_whiteboard":"needle","status_whiteboard_type":"equals"}}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" bug-tags bug_tags \
        '{"api_key":"fixture-secret","bug_id":36,"tag":"probe"}' \
        '{"result":{"bugs":[{"id":201,"request":{"builder":"query","tags":["probe"]}}],"update":{"add":["probe"],"ids":[36],"remove":null}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" comment-add comment_add \
        '{"api_key":"fixture-secret","transport":"REST","bug_id":41,"text":"hello","is_private":true}' \
        '{"result":{"ids":[41],"update":{"builder":"update","comment":{"comment":"hello","is_private":true}}},"transport":"REST"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" comment-list comment_list \
        '{"api_key":"fixture-secret","transport":"XMLRPC","bug_id":41}' \
        '{"result":{"bugs":{"41":{"comments":[{"id":350,"tags":[],"text":"tagged comment"}]}}},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-upload attachment_upload \
        '{"api_key":"fixture-secret","transport":"REST","bug_ids":[41],"source":"/work/compare/attachment.txt","summary":"Fixture","file_name":"attachment.txt","content_type":"text/plain","comment":"uploaded","is_private":false}' \
        '{"result":{"attachment_ids":[451]},"transport":"REST"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-list attachment_list \
        '{"api_key":"fixture-secret","transport":"REST","bug_ids":[41]}' \
        '{"result":{"attachment_ids":null,"attachments":{"451":{"id":451}},"bugs":[41]},"transport":"REST"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-get attachment_get \
        '{"api_key":"fixture-secret","transport":"XMLRPC","attachment_ids":[451]}' \
        '{"result":{"attachment_ids":[451],"attachments":{"451":{"id":451}},"bugs":null},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-download attachment_download \
        '{"api_key":"fixture-secret","transport":"REST","attachment_id":451,"destination":"/work/compare/download.txt"}' \
        '{"result":{"attachment_id":451,"bytes":19},"transport":"REST"}'
    assert_equals 'fixture attachment' "$(<"$config_dir/compare/download.txt")" \
        "adapter attachment download bytes"
    assert_equals 600 \
        "$("$runtime" exec "$sidecar" stat -c '%a' /work/compare/download.txt)" \
        "adapter attachment download mode"
    mkdir -p "$config_dir/compare/cli-download"
    printf 'sentinel\n' >"$config_dir/compare/cli-download/current.txt"
    chmod 600 "$config_dir/compare/cli-download/current.txt"
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-cli-download \
        attachment_cli_download_bug \
        '{"api_key":"fixture-secret","transport":"REST","bug_id":41,"destination":"/work/compare/cli-download","ignore_obsolete":true}' \
        '{"result":{"bug_id":41,"files":["current-1.txt"]},"transport":"REST"}'
    assert_equals 'sentinel' \
        "$(<"$config_dir/compare/cli-download/current.txt")" \
        "adapter CLI attachment collision sentinel"
    assert_equals 'fixture attachment' \
        "$(<"$config_dir/compare/cli-download/current-1.txt")" \
        "adapter CLI attachment download bytes"
    assert_equals 700 \
        "$("$runtime" exec "$sidecar" stat -c '%a' /work/compare/cli-download)" \
        "adapter CLI attachment directory mode"
    assert_equals 600 \
        "$("$runtime" exec "$sidecar" stat -c '%a' /work/compare/cli-download/current-1.txt)" \
        "adapter CLI attachment file mode"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" attachment-cli-unsafe \
        attachment_cli_download_bug \
        '{"api_key":"fixture-secret","transport":"REST","bug_id":42,"destination":"/work/compare/cli-unsafe","ignore_obsolete":true}' \
        'python-bugzilla returned an unsafe attachment name'
    if [[ -e $config_dir/compare/outside.txt ]]; then
        printf 'unsafe CLI attachment escaped its destination\n' >&2
        return 1
    fi
    ln -s cli-download "$config_dir/compare/cli-link"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" attachment-cli-symlink \
        attachment_cli_download_bug \
        '{"api_key":"fixture-secret","transport":"REST","bug_id":41,"destination":"/work/compare/cli-link","ignore_obsolete":true}' \
        'destination path must not be a symlink'
    printf 'not a directory\n' >"$config_dir/compare/cli-file"
    chmod 600 "$config_dir/compare/cli-file"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" attachment-cli-file \
        attachment_cli_download_bug \
        '{"api_key":"fixture-secret","transport":"REST","bug_id":41,"destination":"/work/compare/cli-file","ignore_obsolete":true}' \
        'destination must be a non-symlink directory'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" attachment-flag attachment_flag \
        '{"api_key":"fixture-secret","transport":"XMLRPC","bug_id":41,"attachment_id":451,"flag_name":"review","status":"?","requestee":"reviewer@test.invalid"}' \
        '{"result":{"attachment_id":451,"bug_id":41,"flag_name":"review","requestee":"reviewer@test.invalid","status":"?"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" user-create user_create \
        '{"api_key":"fixture-secret","email":"fixture-user@test.invalid","name":"Fixture User","password":"secret"}' \
        '{"result":{"can_login":true,"email":"fixture-user@test.invalid","groups":["editbugs"],"id":601,"name":"fixture-user@test.invalid","real_name":"Fixture User"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" user-get user_get \
        '{"api_key":"fixture-secret","email":"fixture-user@test.invalid"}' \
        '{"result":{"can_login":true,"email":"fixture-user@test.invalid","groups":["editbugs"],"id":601,"name":"fixture-user@test.invalid","real_name":"Fixture User"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" user-search user_search \
        '{"api_key":"fixture-secret","pattern":"fixture-user@test.invalid"}' \
        '{"result":[{"can_login":true,"email":"fixture-user@test.invalid","groups":["editbugs"],"id":601,"name":"fixture-user@test.invalid","real_name":"Fixture User"}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" user-groups user_groups \
        '{"api_key":"fixture-secret","email":"fixture-user@test.invalid","action":"add","groups":["editbugs"]}' \
        '{"result":{"action":"add","groups":["editbugs"],"user":"fixture-user@test.invalid"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" group-get group_get \
        '{"api_key":"fixture-secret","name":"editbugs","membership":true}' \
        '{"result":{"description":"Can edit bugs","id":701,"is_active":true,"members":["fixture-user@test.invalid"],"name":"editbugs"},"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" group-list group_list \
        '{"api_key":"fixture-secret","names":["editbugs"],"membership":true}' \
        '{"result":[{"description":"Can edit bugs","id":701,"is_active":true,"members":["fixture-user@test.invalid"],"name":"editbugs"}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" product-catalogue product_catalogue \
        '{"api_key":"fixture-secret","catalogue":"enterable"}' \
        '{"result":[{"id":801,"name":"enterable"}],"transport":"XMLRPC"}'
    assert_adapter_case "$runtime" "$sidecar" "$config_dir" component-add component_add \
        '{"api_key":"fixture-secret","params":{"product":"Widget","name":"Core","description":"Core component","default_assignee":"admin@test.invalid"}}' \
        '{"result":{"id":901,"request":{"default_assignee":"admin@test.invalid","description":"Core component","name":"Core","product":"Widget"}},"transport":"XMLRPC"}'

    printf '%s\n' \
        '{"api_key":"fixture-secret","params":{"product":"Widget","component":"Core","initialowner":"admin@test.invalid","description":"Updated"}}' \
        >"$local_input"
    chmod 600 "$local_input"
    "$runtime" exec "$sidecar" python /work/compare/python-bugzilla-adapter.py \
        component_update_shape /work/compare/component-update-local.input.json \
        /work/compare/component-update-local.output.json
    assert_equals \
        '{"result":{"request":{"names":[{"component":"Core","product":"Widget"}],"updates":{"default_assignee":"admin@test.invalid","description":"Updated"}}},"transport":null}' \
        "$(jq -cS . "$local_output")" "adapter local component-update shape"

    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" invalid-transport \
        comment_list \
        '{"api_key":"fixture-secret","transport":"hybrid","bug_id":41}' \
        'transport must be REST or XMLRPC'
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" unknown-key \
        user_get \
        '{"api_key":"fixture-secret","email":"fixture-user@test.invalid","extra":true}' \
        'unexpected request fields: extra'
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" outside-attachment \
        attachment_upload \
        '{"api_key":"fixture-secret","bug_ids":[41],"source":"/work/outside.txt","summary":"Fixture","file_name":"attachment.txt","content_type":"text/plain","comment":"uploaded","is_private":false}' \
        'source path is outside the exchange directory'
    ln -s attachment.txt "$config_dir/compare/attachment-link.txt"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" symlink-attachment \
        attachment_upload \
        '{"api_key":"fixture-secret","bug_ids":[41],"source":"/work/compare/attachment-link.txt","summary":"Fixture","file_name":"attachment.txt","content_type":"text/plain","comment":"uploaded","is_private":false}' \
        'source path must not be a symlink'
    cp "$config_dir/compare/attachment.txt" "$config_dir/compare/public-attachment.txt"
    chmod 644 "$config_dir/compare/public-attachment.txt"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" public-attachment \
        attachment_upload \
        '{"api_key":"fixture-secret","bug_ids":[41],"source":"/work/compare/public-attachment.txt","summary":"Fixture","file_name":"attachment.txt","content_type":"text/plain","comment":"uploaded","is_private":false}' \
        'source file mode must be 0600'
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" upstream-error \
        attachment_download \
        '{"api_key":"fixture-secret","attachment_id":999,"destination":"/work/compare/upstream.txt"}' \
        'operation failed (RuntimeError)'
    printf 'sentinel\n' >"$config_dir/compare/sentinel.txt"
    ln -s sentinel.txt "$config_dir/compare/output-link.txt"
    assert_adapter_rejection "$runtime" "$sidecar" "$config_dir" output-symlink \
        attachment_download \
        '{"api_key":"fixture-secret","attachment_id":451,"destination":"/work/compare/output-link.txt"}' \
        'destination path must not be a symlink'
    assert_equals sentinel "$(<"$config_dir/compare/sentinel.txt")" \
        "adapter output symlink sentinel"

    if "$runtime" exec -e PYTHONPATH=/work/adapter-fixture -e FIXTURE_UNKNOWN_BACKEND=1 \
        "$sidecar" python /work/compare/python-bugzilla-adapter.py create \
        /work/compare/create.input.json /work/compare/unknown-backend.output.json \
        2>"$error_output"; then
        printf 'adapter accepted an unknown backend class\n' >&2
        return 1
    fi
    if ! grep -Fq 'unsupported python-bugzilla backend: _UnknownBackend' "$error_output"; then
        printf 'adapter unknown-backend rejection omitted its diagnostic\n' >&2
        return 1
    fi

    printf '%s\n' '{"api_key":"fixture-secret","bug_id":0}' >"$invalid_input"
    chmod 600 "$invalid_input"
    set +e
    "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/compare/python-bugzilla-adapter.py view \
        /work/compare/invalid-id.input.json /work/compare/invalid-id.output.json \
        2>"$error_output"
    invalid_status=$?
    set -e
    assert_equals 1 "$invalid_status" "invalid adapter ID status"
    if grep -Fq fixture-secret "$error_output"; then
        printf 'adapter error leaked the API key\n' >&2
        return 1
    fi
    if grep -Fq '/work/compare/invalid-id.input.json' "$error_output"; then
        printf 'adapter error leaked the input path\n' >&2
        return 1
    fi
    if "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/compare/python-bugzilla-adapter.py unsupported \
        /work/compare/invalid-id.input.json /work/compare/unsupported.output.json \
        2>"$error_output"; then
        printf 'adapter accepted an unsupported operation\n' >&2
        return 1
    fi
    if "$runtime" exec -e PYTHONPATH=/work/adapter-fixture "$sidecar" \
        python /work/compare/python-bugzilla-adapter.py view \
        /work/compare/invalid-id.input.json \
        2>"$error_output"; then
        printf 'adapter accepted an incomplete argument list\n' >&2
        return 1
    fi
}

run_comment_phase_fixture() (
    local phase="$PYBZ_DIR/../compare/02-comments.sh"
    local fixture_output
    local counter_file

    if [[ ! -r $phase ]] || ! declare -F resource_init >/dev/null; then
        printf 'missing resource helper or comment comparison phase\n' >&2
        return 1
    fi

    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    counter_file="$COMPARE_EXCHANGE_DIR/fixture-next-id"
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT
    printf '100\n' >"$counter_file"
    TEST_ID_PREFIX=compare CURRENT_TEST_GROUP=02-comments BZ_VERSION=bz50
    BZ_URL=http://127.0.0.1 BZR_COMPARE_API_KEY=fixture-secret
    COMPARE_ADMIN_EMAIL=admin@test.bzr

    reset_comment_fixture() {
        PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
        SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 RESOURCE_GAP_ELIGIBLE=0
        rm -f "$COMPARE_EXCHANGE_DIR"/fixture-comment-*.json
        printf '100\n' >"$counter_file"
        : >"$fixture_output"
    }
    comment_fixture_transport() {
        local api="$1"
        if [[ $api == xmlrpc ]]; then
            printf 'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call\n'
        else
            printf 'DEBUG bzr::client::transport: API response\n'
        fi
    }
    run_bzr() {
        local args=("$@") api=rest command='' id='' text='' private=false index next
        local state
        BZR_EXIT=0
        for ((index = 0; index < ${#args[@]}; index++)); do
            next=$((index + 1))
            case ${args[index]} in
            --api) api=${args[next]} ;;
            --body) text=${args[next]} ;;
            --private) private=true ;;
            bug)
                [[ ${args[next]:-} == create ]] && command=bug-create
                ;;
            comment)
                if [[ ${args[next]:-} == add ]]; then
                    command='comment-add'
                    id=${args[index + 2]}
                elif [[ ${args[next]:-} == list ]]; then
                    command='comment-list'
                    id=${args[index + 2]}
                fi
                ;;
            config)
                [[ ${args[next]:-} == set-server ]] && command=config-set-server
                ;;
            esac
        done
        case $command in
        config-set-server)
            if [[ ${COMMENT_CONFIG_FAILURE:-0} -eq 1 ]]; then
                BZR_EXIT=1
            else
                RESOURCE_QUERY_AUTH_CONFIGURED=1
                printf '{}\n' >"$BZR_STDOUT"
            fi
            ;;
        bug-create)
            if [[ ${COMMENT_CREATE_FAILURE:-0} -eq 1 ]]; then
                printf 'controlled bug create failure\n' >"$BZR_STDERR"
                BZR_EXIT=1
            elif [[ ${COMMENT_CREATE_NONPOSITIVE_ID:-0} -eq 1 ]]; then
                printf '{"id":0}\n' >"$BZR_STDOUT"
            else
                id=$(<"$counter_file")
                printf '%s\n' "$((id + 1))" >"$counter_file"
                jq -cn --argjson id "$id" '{id:$id}' >"$BZR_STDOUT"
            fi
            ;;
        comment-add)
            state="$COMPARE_EXCHANGE_DIR/fixture-comment-${id}.json"
            jq -cn --arg text "$text" --argjson private "$private" \
                '{id:350,text:$text,is_private:$private}' >"$state"
            printf '{"id":350}\n' >"$BZR_STDOUT"
            ;;
        comment-list)
            state="$COMPARE_EXCHANGE_DIR/fixture-comment-${id}.json"
            if [[ ${RESOURCE_QUERY_AUTH_CONFIGURED:-0} -ne 1 && $api == rest && -r $state ]] &&
                jq -e '.is_private == true' "$state" >/dev/null; then
                printf '[]\n' >"$BZR_STDOUT"
            elif [[ -r $state ]]; then
                jq -s '.' "$state" >"$BZR_STDOUT"
            else
                printf '[]\n' >"$BZR_STDOUT"
            fi
            ;;
        *) return 2 ;;
        esac
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        if [[ ${COMMENT_TRANSPORT_MISSING:-0} -eq 1 && $command == comment-list ]]; then
            : >"$BZR_STDERR"
        else
            comment_fixture_transport "$api" >"$BZR_STDERR"
        fi
    }
    run_pybz_adapter() {
        local operation="$1" input_name="${2##*/}" output_name="${3##*/}"
        local input="$COMPARE_EXCHANGE_DIR/$input_name"
        local output="$COMPARE_EXCHANGE_DIR/$output_name"
        local transport id state comment
        transport=$(jq -r '.transport' "$input")
        id=$(jq -r '.bug_id' "$input")
        state="$COMPARE_EXCHANGE_DIR/fixture-comment-${id}.json"
        case $operation in
        comment_add)
            jq '{id:351,text:.text,is_private:.is_private}' "$input" >"$state"
            jq -cn --arg transport "$transport" \
                '{transport:$transport,result:{ids:[351]}}' >"$output"
            ;;
        comment_list)
            if [[ ${COMMENT_MISSING_RECORD:-0} -eq 1 ]]; then
                comment='null'
            else
                comment=$(<"$state")
                if [[ ${COMMENT_PRIVACY_FLIPPED:-0} -eq 1 ]] &&
                    jq -e '.is_private == true' "$state" >/dev/null; then
                    comment=$(jq '.is_private = false' "$state")
                fi
            fi
            jq -cn --arg transport "$transport" --argjson id "$id" \
                --argjson comment "$comment" \
                '{transport:$transport,result:{bugs:{($id|tostring):
                  {comments:(if $comment == null then [] else [$comment] end)}}}}' >"$output"
            ;;
        *) return 2 ;;
        esac
        : >"$BZR_STDOUT"
        : >"$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT=0
    }
    run_comment_control() {
        local flag="$1" slug="$2"
        reset_comment_fixture
        printf -v "$flag" 1
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag"
        if [[ $FAIL_COUNT -eq 0 ]] ||
            ! grep -Fq "[compare/02-comments/${slug}]" "$fixture_output"; then
            printf 'comment control %s unexpectedly passed\n' "$flag" >&2
            return 1
        fi
        printf 'controlled red: comments %s=1\n' "$flag"
    }

    COMMENT_CONFIG_FAILURE=1
    if resource_init; then
        printf 'resource_init accepted a failed comparison-server setup\n' >&2
        return 1
    fi
    unset COMMENT_CONFIG_FAILURE
    resource_init
    reset_comment_fixture
    # shellcheck source=tests/functional/compare/02-comments.sh
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    assert_equals 3 "$PASS_COUNT" "comment comparison pass count"
    assert_equals 0 "$FAIL_COUNT" "comment comparison fail count"
    for slug in public-comments private-comments-rest private-comments-xmlrpc; do
        if ! grep -Fq "[compare/02-comments/${slug}]" "$fixture_output"; then
            printf 'comment phase omitted stable ID: %s\n' "$slug" >&2
            return 1
        fi
    done
    run_comment_control COMMENT_MISSING_RECORD public-comments
    run_comment_control COMMENT_PRIVACY_FLIPPED private-comments-rest
    run_comment_control COMMENT_TRANSPORT_MISSING public-comments
    run_comment_control COMMENT_CREATE_FAILURE public-comments
    run_comment_control COMMENT_CREATE_NONPOSITIVE_ID public-comments
    reset_comment_fixture
    RESOURCE_QUERY_AUTH_CONFIGURED=0
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    if [[ $FAIL_COUNT -eq 0 ]] ||
        ! grep -Fq '[compare/02-comments/private-comments-rest]' "$fixture_output"; then
        printf 'comment query-auth omission unexpectedly passed\n' >&2
        return 1
    fi
    printf 'controlled red: comments query-parameter auth omitted\n'
)

run_attachment_phase_fixture() (
    local phase="$PYBZ_DIR/../compare/03-attachments.sh"
    local fixture_output
    local next_bug next_bzr_attachment next_pybz_attachment

    if [[ ! -r $phase ]]; then
        printf 'missing attachment comparison phase\n' >&2
        return 1
    fi
    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT
    TEST_ID_PREFIX=compare CURRENT_TEST_GROUP=03-attachments BZ_VERSION=bz50
    RESOURCE_SERVER=compare-resource

    eval "$(declare -f expect_gap | sed '1s/expect_gap/attachment_fixture_expect_gap/')"
    expect_gap() {
        local issue="$1"

        if [[ ${ATTACHMENT_GAP_OWNER_FAULT:-0} -eq 1 ]]; then
            issue=999
        fi
        attachment_fixture_expect_gap "$issue"
    }

    reset_attachment_fixture() {
        PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
        SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 GAP_APPLIED=0
        RESOURCE_GAP_ELIGIBLE=0
        RESOURCE_GAP_FILE="$COMPARE_EXCHANGE_DIR/.resource-gap-eligible"
        next_bug=100 next_bzr_attachment=200 next_pybz_attachment=300
        : >"$fixture_output"
    }
    attachment_fixture_write_bzr() {
        local name="$1" payload="$2"
        printf '%s\n' "$payload" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json"
        printf '%s\n' "$payload" >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        printf 'DEBUG bzr::client::transport: API response\n' >"$BZR_STDERR"
        BZR_EXIT=0
    }
    resource_bzr() {
        local name="$1" api="$2" transport="$3" command id out summary private=false path
        shift 3
        command="$*"
        if [[ ${ATTACHMENT_TRANSPORT_FAULT:-0} -eq 1 &&
            $name == private-xmlrpc-bzr-list ]]; then
            test_fail "controlled attachment transport failure"
            return 1
        fi
        if [[ ${ATTACHMENT_DOWNLOAD_COMMAND_FAILURE:-0} -eq 1 &&
            $name == public-bzr-download ]]; then
            test_fail "controlled attachment download command failure"
            return 1
        fi
        case $command in
        "bug create "*)
            attachment_fixture_write_bzr "$name" "{\"id\":$next_bug}"
            next_bug=$((next_bug + 1))
            ;;
        "attachment upload "*)
            attachment_fixture_write_bzr "$name" "{\"id\":$next_bzr_attachment}"
            next_bzr_attachment=$((next_bzr_attachment + 1))
            ;;
        "attachment list "*)
            case $name in
            public-*) summary="$ATTACHMENT_STEM public" ;;
            private-rest-*) summary="$ATTACHMENT_STEM private-rest"; private=true ;;
            private-xmlrpc-*) summary="$ATTACHMENT_STEM private-xmlrpc"; private=true ;;
            esac
            id="$ATTACHMENT_BZR_ID"
            jq -cn --argjson id "$id" --arg summary "$summary" \
                --argjson private "$private" \
                '[{id:$id,file_name:"attachment-source.txt",summary:$summary,
                   content_type:"text/plain",is_private:$private,is_obsolete:false,flags:[]}]' \
                >"$COMPARE_EXCHANGE_DIR/fixture-payload.json"
            attachment_fixture_write_bzr "$name" \
                "$(<"$COMPARE_EXCHANGE_DIR/fixture-payload.json")"
            ;;
        "comment list "*)
            attachment_fixture_write_bzr "$name" \
                "[{\"text\":\"Created attachment\\n\\n$_ATTACH_PUBLIC_COMMENT\"}]"
            ;;
        "attachment view "*)
            if [[ $name == flag-* ]]; then
                jq -cn --argjson id "$ATTACHMENT_BZR_ID" \
                    '{id:$id,flags:[{name:"bzr_compare_attachment_review",status:"?"}]}' \
                    >"$COMPARE_EXCHANGE_DIR/fixture-payload.json"
                attachment_fixture_write_bzr "$name" \
                    "$(<"$COMPARE_EXCHANGE_DIR/fixture-payload.json")"
            else
                attachment_fixture_write_bzr "$name" \
                    "{\"id\":$ATTACHMENT_BZR_ID,\"is_private\":true,\"flags\":[]}"
            fi
            ;;
        "attachment download "*)
            path=''
            while [[ $# -gt 0 ]]; do
                case $1 in
                --out) path="$2"; shift 2 ;;
                --out-dir)
                    path="$2/$ATTACHMENT_BZR_BUG_ID/${ATTACHMENT_BZR_ID}.attachment-source.txt"
                    shift 2
                    ;;
                *) shift ;;
                esac
            done
            mkdir -p "${path%/*}"
            cp "$ATTACHMENT_SOURCE" "$path"
            if [[ $name == public-bzr-bulk ]]; then
                jq -cn --argjson id "$ATTACHMENT_BZR_ID" --arg path "$path" \
                    '{bug_results:[{files:[{attachment_id:$id,path:$path}]}]}' \
                    >"$COMPARE_EXCHANGE_DIR/fixture-payload.json"
                attachment_fixture_write_bzr "$name" \
                    "$(<"$COMPARE_EXCHANGE_DIR/fixture-payload.json")"
            else
                attachment_fixture_write_bzr "$name" '{}'
            fi
            ;;
        "attachment update "*) attachment_fixture_write_bzr "$name" '{}' ;;
        *)
            test_fail "unhandled bzr attachment fixture command"
            return 1
            ;;
        esac
        if [[ $transport == XMLRPC ]]; then
            printf 'DEBUG bzr::xmlrpc::protocol::client: XML-RPC call\n' >"$BZR_STDERR"
        fi
    }
    resource_pybz() {
        local name="$1" operation="$2" payload="$3" transport="$4"
        local result id bug_id summary private=false destination count
        if [[ ${ATTACHMENT_MULTI_COMMAND_FAILURE:-0} -eq 1 &&
            $name == multi-pybz-upload ]] ||
            [[ ${ATTACHMENT_OBSOLETE_COMMAND_FAILURE:-0} -eq 1 &&
                $name == obsolete-pybz-download ]]; then
            test_fail "controlled attachment command failure"
            return 1
        fi
        case $operation in
        attachment_upload)
            count=$(jq '.bug_ids | length' <<<"$payload")
            if [[ $count -eq 2 ]]; then
                result='{"attachment_ids":[901,902]}'
            else
                id=$next_pybz_attachment
                next_pybz_attachment=$((next_pybz_attachment + 1))
                result="{\"attachment_ids\":[$id]}"
            fi
            ;;
        attachment_list)
            bug_id=$(jq -r '.bug_ids[0]' <<<"$payload")
            case $name in
            public-*) summary="$ATTACHMENT_STEM public" ;;
            private-rest-*) summary="$ATTACHMENT_STEM private-rest"; private=true ;;
            private-xmlrpc-*) summary="$ATTACHMENT_STEM private-xmlrpc"; private=true ;;
            obsolete-*) summary="$ATTACHMENT_STEM public" ;;
            esac
            id="$ATTACHMENT_PYBZ_ID"
            [[ ${ATTACHMENT_METADATA_FAULT:-0} -eq 1 && $name == public-* ]] && summary=wrong
            [[ ${ATTACHMENT_PRIVACY_FAULT:-0} -eq 1 && $name == private-rest-* ]] && private=false
            if [[ $name == obsolete-* ]]; then
                result="{\"bugs\":{\"$bug_id\":[{\"id\":$id,\"is_obsolete\":true}]}}"
            else
                result=$(jq -cn --arg bug_id "$bug_id" --argjson id "$id" \
                    --arg summary "$summary" --argjson private "$private" \
                    '{bugs:{($bug_id):[{id:$id,file_name:"attachment-source.txt",
                      summary:$summary,content_type:"text/plain",is_private:$private,
                      is_obsolete:false}]}}')
            fi
            ;;
        comment_list)
            bug_id=$(jq -r '.bug_id' <<<"$payload")
            summary="$_ATTACH_PUBLIC_COMMENT"
            [[ ${ATTACHMENT_COMMENT_FAULT:-0} -eq 1 ]] && summary=wrong
            result=$(jq -cn --arg bug_id "$bug_id" \
                --arg text "Created attachment"$'\n\n'"$summary" \
                '{bugs:{($bug_id):{comments:[{text:$text}]}}}')
            ;;
        attachment_download)
            id=$(jq -r '.attachment_id' <<<"$payload")
            destination="$COMPARE_EXCHANGE_DIR/$(jq -r '.destination | split("/")[-1]' \
                <<<"$payload")"
            if [[ ${ATTACHMENT_DIGEST_FAULT:-0} -eq 1 ]]; then
                printf 'wrong bytes\n' >"$destination"
            else
                cp "$ATTACHMENT_SOURCE" "$destination"
            fi
            result="{\"attachment_id\":$id,\"bytes\":1}"
            ;;
        attachment_cli_download_bug)
            result='{"bug_id":100,"files":["attachment-source.txt"]}'
            ;;
        attachment_flag) result='{}' ;;
        attachment_get)
            id=$(jq -r '.attachment_ids[0]' <<<"$payload")
            if [[ $name == flag-* ]]; then
                if [[ ${ATTACHMENT_FLAG_FAULT:-0} -eq 1 ]]; then
                    result="{\"attachments\":{\"$id\":{\"id\":$id,\"flags\":[]}}}"
                else
                    local requestee=null
                    [[ ${ATTACHMENT_FLAG_EQUALITY_FAULT:-0} -eq 1 ]] &&
                        requestee='"other@test.bzr"'
                    result=$(jq -cn --arg id "$id" \
                        --argjson requestee "$requestee" \
                        '{attachments:{($id):{id:($id|tonumber),
                          flags:[{name:"bzr_compare_attachment_review",status:"?",
                            requestee:$requestee}]}}}')
                fi
            else
                result="{\"attachments\":{\"$id\":{\"id\":$id,\"is_private\":true}}}"
            fi
            ;;
        *)
            test_fail "unhandled python-bugzilla attachment fixture operation"
            return 1
            ;;
        esac
        printf '%s\n' "$result" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.result.json"
        if [[ $transport == XMLRPC ]]; then
            printf 'XMLRPC\n' >"$COMPARE_EXCHANGE_DIR/${name}.pybz.transport"
        else
            printf 'REST\n' >"$COMPARE_EXCHANGE_DIR/${name}.pybz.transport"
        fi
    }
    run_bzr() {
        local command="$*" diagnostic usage
        : >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        if [[ ${ATTACHMENT_GAP_STALE:-0} -eq 1 ]]; then
            BZR_EXIT=0
            : >"$BZR_STDERR"
            return
        fi
        if [[ $command == *'attachment upload'* ]]; then
            diagnostic="error: unexpected argument '$ATTACHMENT_SOURCE' found"
            usage='Usage: bzr attachment upload [OPTIONS] <BUG_ID> <FILE>'
        else
            diagnostic="error: unexpected argument '--ignore-obsolete' found"
            usage='Usage: bzr attachment download --bug <BUG_ID> [ID]...'
        fi
        [[ ${ATTACHMENT_GAP_WRONG_DIAGNOSTIC:-0} -eq 1 ]] && diagnostic='error: unrelated'
        printf '%s\n\n%s\n' "$diagnostic" "$usage" >"$BZR_STDERR"
        BZR_EXIT=2
    }
    run_attachment_control() {
        local flag="$1" slug="$2" expected_failures="${3:-minimum}"
        reset_attachment_fixture
        printf -v "$flag" 1
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag"
        if [[ $FAIL_COUNT -eq 0 ]] ||
            [[ $expected_failures != minimum && $FAIL_COUNT -ne $expected_failures ]] ||
            ! grep -Fq "[compare/03-attachments/${slug}]" "$fixture_output"; then
            printf 'attachment control %s unexpectedly passed\n' "$flag" >&2
            return 1
        fi
        printf 'controlled red: attachments %s=1\n' "$flag"
    }
    attachment_assert_gap_owners() {
        local slug

        for slug in multi-bug-upload ignore-obsolete; do
            if ! grep -Eq \
                "\\[compare/03-attachments/${slug}\\].*GAP \\(#674\\)$" \
                "$fixture_output"; then
                printf 'attachment gap %s did not render owner #674\n' "$slug" >&2
                return 1
            fi
        done
    }

    reset_attachment_fixture
    # shellcheck source=tests/functional/compare/03-attachments.sh
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    assert_equals 5 "$PASS_COUNT" "attachment comparison pass count"
    assert_equals 0 "$FAIL_COUNT" "attachment comparison fail count"
    assert_equals 2 "$GAP_COUNT" "attachment comparison gap count"
    attachment_assert_gap_owners
    for slug in upload-metadata-comment download-content attachment-flags \
        private-attachments-rest private-attachments-xmlrpc multi-bug-upload ignore-obsolete; do
        if ! grep -Fq "[compare/03-attachments/${slug}]" "$fixture_output"; then
            printf 'attachment phase omitted stable ID: %s\n' "$slug" >&2
            return 1
        fi
    done
    run_attachment_control ATTACHMENT_METADATA_FAULT upload-metadata-comment
    run_attachment_control ATTACHMENT_COMMENT_FAULT upload-metadata-comment
    run_attachment_control ATTACHMENT_DIGEST_FAULT download-content
    run_attachment_control ATTACHMENT_FLAG_FAULT attachment-flags
    run_attachment_control ATTACHMENT_FLAG_EQUALITY_FAULT attachment-flags 1
    run_attachment_control ATTACHMENT_PRIVACY_FAULT private-attachments-rest
    run_attachment_control ATTACHMENT_TRANSPORT_FAULT private-attachments-xmlrpc
    run_attachment_control ATTACHMENT_GAP_WRONG_DIAGNOSTIC multi-bug-upload
    run_attachment_control ATTACHMENT_GAP_STALE multi-bug-upload
    run_attachment_control ATTACHMENT_MULTI_COMMAND_FAILURE multi-bug-upload 1
    run_attachment_control ATTACHMENT_OBSOLETE_COMMAND_FAILURE ignore-obsolete 1
    reset_attachment_fixture
    ATTACHMENT_DOWNLOAD_COMMAND_FAILURE=1
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    unset ATTACHMENT_DOWNLOAD_COMMAND_FAILURE
    assert_equals 1 "$FAIL_COUNT" "attachment download command failure count"
    if ! grep -Fq '[compare/03-attachments/download-content]' "$fixture_output"; then
        printf 'attachment download command failure omitted its stable ID\n' >&2
        return 1
    fi
    printf 'controlled red: attachment download command failure counted once\n'
    reset_attachment_fixture
    ATTACHMENT_GAP_OWNER_FAULT=1
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    unset ATTACHMENT_GAP_OWNER_FAULT
    if attachment_assert_gap_owners; then
        printf 'attachment wrong-owner control unexpectedly passed\n' >&2
        return 1
    fi
    printf 'controlled red: attachments wrong gap owner\n'
)

run_attachment_seed_fixture() (
    local seed_dir

    seed_dir=$(mktemp -d)
    trap 'rm -rf "$seed_dir"' EXIT
    COMPARE_EXCHANGE_DIR="$seed_dir"
    run_bugzilla_sql_file() {
        local sql_file="$1"
        if [[ ! -r $sql_file ]] ||
            ! grep -Fq "WHERE NOT EXISTS" "$sql_file" ||
            ! grep -Fq "flaginclusions.product_id IS NULL" "$sql_file" ||
            ! grep -Fq "flaginclusions.component_id IS NULL" "$sql_file" ||
            ! grep -Fq "AS unrestricted_inclusion_count" "$sql_file"; then
            return 1
        fi
        : >"$seed_dir/sql-seen"
        case ${ATTACHMENT_SEED_MODE:-green} in
        green) printf 'flag_type_count\tunrestricted_inclusion_count\n1\t1\n' ;;
        command-failure) return 1 ;;
        bad-readback) printf 'flag_type_count\tunrestricted_inclusion_count\n0\t0\n' ;;
        restricted-only) printf 'flag_type_count\tunrestricted_inclusion_count\n1\t0\n' ;;
        esac
    }

    seed_comparison_attachment_flag_type
    if [[ ! -f $seed_dir/sql-seen ]]; then
        printf 'attachment flag seed did not invoke SQL\n' >&2
        return 1
    fi
    if [[ -e $seed_dir/attachment-flag.sql ]]; then
        printf 'attachment seed retained its SQL file\n' >&2
        return 1
    fi
    ATTACHMENT_SEED_MODE='command-failure'
    if seed_comparison_attachment_flag_type; then
        printf 'attachment seed accepted command failure\n' >&2
        return 1
    fi
    if [[ -e $seed_dir/attachment-flag.sql ]]; then
        printf 'attachment seed retained SQL after command failure\n' >&2
        return 1
    fi
    ATTACHMENT_SEED_MODE='bad-readback'
    if seed_comparison_attachment_flag_type; then
        printf 'attachment seed accepted bad readback\n' >&2
        return 1
    fi
    if [[ -e $seed_dir/attachment-flag.sql ]]; then
        printf 'attachment seed retained SQL after bad readback\n' >&2
        return 1
    fi
    ATTACHMENT_SEED_MODE='restricted-only'
    if seed_comparison_attachment_flag_type; then
        printf 'attachment seed accepted a restricted-only inclusion\n' >&2
        return 1
    fi
    if [[ -e $seed_dir/attachment-flag.sql ]]; then
        printf 'attachment seed retained SQL after restricted readback\n' >&2
        return 1
    fi
)

run_user_group_phase_fixture() (
    local phase="$PYBZ_DIR/../compare/04-users-groups.sh"
    local fixture_output

    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT
    TEST_ID_PREFIX=compare CURRENT_TEST_GROUP=04-users-groups BZ_VERSION=bz50
    RESOURCE_SERVER=compare-resource

    reset_user_group_fixture() {
        PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
        SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 GAP_APPLIED=0
        RESOURCE_MEMBERSHIPS="" USER_GROUP_BZR_MEMBER=0 USER_GROUP_PYBZ_MEMBER=0
        USER_GROUP_BZR_TRANSPORTS=""
        USER_GROUP_PYBZ_TRANSPORTS=""
        : >"$fixture_output"
    }
    user_group_fixture_bzr_output() {
        printf '%s\n' "$2" >"$COMPARE_EXCHANGE_DIR/${1}.bzr.stdout.json"
    }
    resource_bzr() {
        local name="$1" api="$2" expected_transport="$3" command
        USER_GROUP_BZR_TRANSPORTS+="${name}:${api}:${expected_transport}"$'\n'
        shift 3
        command="$*"
        case $name in
        user-bzr-create)
            if [[ ${USER_GROUP_BZR_USER_ID_FAULT:-0} -eq 1 ]]; then
                user_group_fixture_bzr_output "$name" '{"id":0}'
            else
                user_group_fixture_bzr_output "$name" '{"id":101}'
            fi
            ;;
        user-bzr-search)
            if [[ ${USER_GROUP_SEARCH_FAULT:-0} -eq 1 ]]; then
                user_group_fixture_bzr_output "$name" '[]'
            else
                user_group_fixture_bzr_output "$name" \
                    "[{\"name\":\"$USER_GROUP_BZR_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":[]}]"
            fi
            ;;
        group-bzr-create)
            if [[ ${USER_GROUP_GROUP_ID_FAULT:-0} -eq 1 ]]; then
                user_group_fixture_bzr_output "$name" '{"id":0}'
            else
                user_group_fixture_bzr_output "$name" '{"id":301}'
            fi
            ;;
        group-bzr-view)
            user_group_fixture_bzr_output "$name" \
                "{\"name\":\"$USER_GROUP_FIXTURE\",\"description\":\"$USER_GROUP_DESCRIPTION\",\"is_active\":true}"
            ;;
        membership-bzr-add)
            USER_GROUP_BZR_MEMBER=1
            user_group_fixture_bzr_output "$name" '{}'
            ;;
        membership-bzr-remove)
            USER_GROUP_BZR_MEMBER=0
            user_group_fixture_bzr_output "$name" '{}'
            ;;
        membership-bzr-read)
            local email="$USER_GROUP_BZR_EMAIL" groups='[]'
            [[ ${USER_GROUP_NONMEMBER_FAULT:-0} -eq 1 ]] && email='substitute@test.bzr'
            [[ $USER_GROUP_BZR_MEMBER -eq 1 ]] && groups="[\"$USER_GROUP_FIXTURE\"]"
            user_group_fixture_bzr_output "$name" \
                "[{\"name\":\"$email\",\"real_name\":\"\",\"can_login\":true,\"groups\":$groups}]"
            ;;
        *)
            printf 'unhandled user/group bzr fixture command: %s\n' "$command" >&2
            return 1
            ;;
        esac
    }
    resource_pybz() {
        local name="$1" operation="$2" payload="$3" expected_transport="$4"
        local result groups='[]'
        USER_GROUP_PYBZ_TRANSPORTS+="${name}:${expected_transport}"$'\n'
        if [[ ${USER_GROUP_MEMBERSHIP_COMMAND_FAILURE:-0} -eq 1 &&
            $name == membership-pybz-remove ]]; then
            test_fail "controlled membership command failure"
            return 1
        fi
        case $name in
        user-pybz-create)
            if [[ ${USER_GROUP_PYBZ_USER_ID_FAULT:-0} -eq 1 ]]; then
                result="{\"id\":0,\"email\":\"$USER_GROUP_PYBZ_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":[]}"
            else
                result="{\"id\":201,\"email\":\"$USER_GROUP_PYBZ_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":[]}"
            fi
            ;;
        user-pybz-get)
            result="{\"id\":201,\"email\":\"$USER_GROUP_PYBZ_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":[]}"
            ;;
        user-pybz-search)
            result="[{\"id\":201,\"email\":\"$USER_GROUP_PYBZ_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":[]}]"
            ;;
        group-pybz-get)
            result="{\"name\":\"$USER_GROUP_FIXTURE\",\"description\":\"$USER_GROUP_DESCRIPTION\",\"is_active\":true}"
            ;;
        group-pybz-list)
            result="[{\"name\":\"$USER_GROUP_FIXTURE\",\"description\":\"$USER_GROUP_DESCRIPTION\",\"is_active\":true}]"
            ;;
        membership-pybz-add)
            USER_GROUP_PYBZ_MEMBER=1
            result='{}'
            ;;
        membership-pybz-remove)
            [[ ${USER_GROUP_RETAIN_FAULT:-0} -eq 1 ]] || USER_GROUP_PYBZ_MEMBER=0
            result='{}'
            ;;
        membership-pybz-read)
            [[ $USER_GROUP_PYBZ_MEMBER -eq 1 ]] && groups="[\"$USER_GROUP_FIXTURE\"]"
            result="{\"id\":201,\"email\":\"$USER_GROUP_PYBZ_EMAIL\",\"real_name\":\"\",\"can_login\":true,\"groups\":$groups}"
            ;;
        *)
            printf 'unhandled user/group python fixture operation: %s %s\n' \
                "$operation" "$payload" >&2
            return 1
            ;;
        esac
        printf '%s\n' "$result" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.result.json"
    }
    run_user_group_control() {
        local flag="$1" slug="$2"
        reset_user_group_fixture
        printf -v "$flag" 1
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag"
        if [[ $FAIL_COUNT -ne 1 ]] ||
            ! grep -Fq "[compare/04-users-groups/${slug}]" "$fixture_output"; then
            printf 'user/group control %s unexpectedly passed\n' "$flag" >&2
            return 1
        fi
        printf 'controlled red: users/groups %s=1\n' "$flag"
    }

    reset_user_group_fixture
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    assert_equals 3 "$PASS_COUNT" "user/group comparison pass count"
    assert_equals 0 "$FAIL_COUNT" "user/group comparison fail count"
    assert_equals '' "$RESOURCE_MEMBERSHIPS" "user/group cleanup registry"
    if [[ $USER_GROUP_BZR_TRANSPORTS != *$'group-bzr-view:xmlrpc:XMLRPC\n'* ]]; then
        printf 'group view did not force XML-RPC transport\n' >&2
        return 1
    fi
    if [[ $USER_GROUP_PYBZ_TRANSPORTS != *$'group-pybz-get:XMLRPC\n'* ||
        $USER_GROUP_PYBZ_TRANSPORTS != *$'group-pybz-list:XMLRPC\n'* ]]; then
        printf 'python-bugzilla group reads did not force XML-RPC transport\n' >&2
        return 1
    fi
    run_user_group_control USER_GROUP_SEARCH_FAULT user-create-get-search
    run_user_group_control USER_GROUP_BZR_USER_ID_FAULT user-create-get-search
    run_user_group_control USER_GROUP_PYBZ_USER_ID_FAULT user-create-get-search
    run_user_group_control USER_GROUP_GROUP_ID_FAULT group-get-and-list
    run_user_group_control USER_GROUP_RETAIN_FAULT membership-add-remove
    run_user_group_control USER_GROUP_NONMEMBER_FAULT membership-add-remove
    run_user_group_control USER_GROUP_MEMBERSHIP_COMMAND_FAILURE membership-add-remove
)

run_membership_cleanup_fixture() (
    local calls

    calls=$(mktemp)
    trap 'rm -f "$calls"' EXIT
    RESOURCE_MEMBERSHIPS=""
    resource_membership_record first@test.bzr editbugs
    resource_membership_record second@test.bzr editbugs
    run_bzr() {
        printf '%s\n' "$*" >>"$calls"
        BZR_EXIT=${MEMBERSHIP_CLEANUP_EXIT:-0}
    }
    resource_membership_cleanup
    assert_equals 2 "$(wc -l <"$calls" | tr -d ' ')" "membership cleanup call count"
    if ! grep -Fxq -- \
        '--server compare-resource group remove-user --group editbugs --user first@test.bzr' \
        "$calls"; then
        printf 'membership cleanup omitted the first exact removal\n' >&2
        return 1
    fi
    resource_membership_record failed@test.bzr editbugs
    MEMBERSHIP_CLEANUP_EXIT=1
    if resource_membership_cleanup; then
        printf 'membership cleanup accepted a removal failure\n' >&2
        return 1
    fi
    printf 'controlled red: membership cleanup failure\n'
)

run_product_component_phase_fixture() (
    local phase="$PYBZ_DIR/../compare/05-products-components.sh"
    local fixture_output

    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT
    TEST_ID_PREFIX=compare CURRENT_TEST_GROUP=05-products-components BZ_VERSION=bz50
    RESOURCE_SERVER=compare-resource
    COMPARE_ADMIN_EMAIL=admin@test.bzr
    RESOURCE_GAP_FILE="$COMPARE_EXCHANGE_DIR/.resource-gap-eligible"
    eval "$(declare -f expect_gap | sed '1s/expect_gap/product_fixture_expect_gap/')"
    expect_gap() {
        local issue="$1"
        [[ ${PRODUCT_GAP_OWNER_FAULT:-0} -eq 1 ]] && issue=999
        product_fixture_expect_gap "$issue"
    }
    reset_product_fixture() {
        PASS_COUNT=0 FAIL_COUNT=0 SKIP_COUNT=0 GAP_COUNT=0
        SEEN_TEST_IDS=$'\n' TEST_RESULT_PENDING=0 GAP_APPLIED=0
        RESOURCE_GAP_ELIGIBLE=0
        rm -f "$RESOURCE_GAP_FILE"
        : >"$fixture_output"
    }
    product_fixture_bzr_output() {
        printf '%s\n' "$2" >"$COMPARE_EXCHANGE_DIR/${1}.bzr.stdout.json"
    }
    resource_bzr() {
        local name="$1"
        case $name in
        catalogue-*-bzr)
            if [[ ${PRODUCT_CATALOGUE_FAULT:-0} -eq 1 && $name == catalogue-enterable-bzr ]]; then
                product_fixture_bzr_output "$name" '[{"name":"Other"}]'
            else
                product_fixture_bzr_output "$name" '[{"name":"TestProduct"}]'
            fi
            ;;
        component-*-product) product_fixture_bzr_output "$name" '{"id":301}' ;;
        component-bzr-create)
            if [[ ${PRODUCT_BZR_COMPONENT_ID_FAULT:-0} -eq 1 ]]; then
                product_fixture_bzr_output "$name" '{"id":0}'
            else
                product_fixture_bzr_output "$name" '{"id":401}'
            fi
            ;;
        component-*-view)
            local description="$COMPONENT_DESCRIPTION"
            [[ ${PRODUCT_COMPONENT_FAULT:-0} -eq 1 && $name == component-pybz-view ]] &&
                description=wrong
            product_fixture_bzr_output "$name" \
                "{\"name\":\"$COMPONENT_NAME\",\"description\":\"$description\",\"default_assignee\":\"admin@test.bzr\",\"is_active\":true}"
            ;;
        *) return 1 ;;
        esac
    }
    resource_pybz() {
        local name="$1" operation="$2" result
        case $operation in
        product_catalogue) result='[{"name":"TestProduct"}]' ;;
        component_add)
            if [[ ${PRODUCT_PYBZ_COMPONENT_ID_FAULT:-0} -eq 1 ]]; then
                result='{"id":0}'
            else
                result='{"id":501}'
            fi
            ;;
        component_update_shape)
            if [[ ${PRODUCT_SHAPE_FAULT:-0} -eq 1 ]]; then
                result='{"request":{"names":[],"updates":{}}}'
            else
                result=$(jq -cn --arg product "$PRODUCT_PYBZ_NAME" \
                    --arg component "$COMPONENT_NAME" \
                    '{request:{names:[{product:$product,component:$component}],
                      updates:{default_assignee:"admin@test.bzr",
                        description:"updated comparison component",is_active:false}}}')
            fi
            ;;
        *) return 1 ;;
        esac
        printf '%s\n' "$result" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.result.json"
    }
    run_bzr() {
        : >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        if [[ ${PRODUCT_GAP_STALE:-0} -eq 1 ]]; then
            BZR_EXIT=0
            : >"$BZR_STDERR"
        else
            BZR_EXIT=2
            local diagnostic="error: unrecognized subcommand 'update'"
            [[ ${PRODUCT_GAP_WRONG_DIAGNOSTIC:-0} -eq 1 ]] && diagnostic='error: unrelated'
            printf "%s\n\n%s\n" "$diagnostic" \
                'Usage: bzr component [OPTIONS] <COMMAND>' >"$BZR_STDERR"
        fi
    }
    product_assert_gap_owner() {
        grep -Eq \
            '\[compare/05-products-components/component-update-redhat\].*GAP \(#675\)$' \
            "$fixture_output"
    }
    run_product_control() {
        local flag="$1" slug="$2"
        reset_product_fixture
        printf -v "$flag" 1
        source "$phase" >"$fixture_output"
        _render_test_result >>"$fixture_output"
        unset "$flag"
        if [[ $FAIL_COUNT -ne 1 ]] ||
            ! grep -Fq "[compare/05-products-components/${slug}]" "$fixture_output"; then
            printf 'product/component control %s unexpectedly passed\n' "$flag" >&2
            return 1
        fi
        printf 'controlled red: products/components %s=1\n' "$flag"
    }

    reset_product_fixture
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    assert_equals 2 "$PASS_COUNT" "product/component comparison pass count"
    assert_equals 0 "$FAIL_COUNT" "product/component comparison fail count"
    assert_equals 1 "$GAP_COUNT" "product/component comparison gap count"
    product_assert_gap_owner
    run_product_control PRODUCT_CATALOGUE_FAULT product-catalogues
    run_product_control PRODUCT_COMPONENT_FAULT component-create
    run_product_control PRODUCT_BZR_COMPONENT_ID_FAULT component-create
    run_product_control PRODUCT_PYBZ_COMPONENT_ID_FAULT component-create
    run_product_control PRODUCT_SHAPE_FAULT component-update-redhat
    run_product_control PRODUCT_GAP_WRONG_DIAGNOSTIC component-update-redhat
    run_product_control PRODUCT_GAP_STALE component-update-redhat
    reset_product_fixture
    PRODUCT_GAP_OWNER_FAULT=1
    source "$phase" >"$fixture_output"
    _render_test_result >>"$fixture_output"
    unset PRODUCT_GAP_OWNER_FAULT
    if product_assert_gap_owner; then
        printf 'component-update wrong-owner control unexpectedly passed\n' >&2
        return 1
    fi
    printf 'controlled red: products/components wrong gap owner\n'
)

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

run_container_fixture() (
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
    trap 'cleanup_container_fixture "$runtime" "$donor" "$config_dir"' EXIT

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
)

run_expected_gap_fixture
run_summary_fixture
run_product_normalization_fixture
run_sidecar_wrapper_fixture
run_transport_observation_fixture
run_lifecycle_phase_fixture
run_parity_report_fixture
run_sidecar_stop_failure_fixture
run_adapter_staging_cleanup_fixture
run_comment_phase_fixture
run_attachment_seed_fixture
run_attachment_phase_fixture
run_user_group_phase_fixture
run_membership_cleanup_fixture
run_product_component_phase_fixture
run_container_fixture
