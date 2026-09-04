#!/bin/bash
# Semantic lifecycle comparisons sourced by run-compare.sh.

printf -v LIFECYCLE_RUN_TOKEN '%x-%x-%x' "$$" "$RANDOM" "$RANDOM"
LIFECYCLE_RUN_TOKEN="${LIFECYCLE_RUN_TOKEN:0:18}"
LIFECYCLE_STEM="bzr-pybz-lifecycle-${BZ_VERSION}-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_BZR_SUMMARY="$LIFECYCLE_STEM [bzr]"
LIFECYCLE_PYBZ_SUMMARY="$LIFECYCLE_STEM [pybz]"
LIFECYCLE_UPDATED_SUMMARY="$LIFECYCLE_STEM updated"
LIFECYCLE_DESCRIPTION="lifecycle description"
LIFECYCLE_URL="https://example.test/updated"
LIFECYCLE_WHITEBOARD="updated"
LIFECYCLE_UPDATED_SEVERITY="major"
LIFECYCLE_UPDATED_PRIORITY="High"
LIFECYCLE_BZR_ID=""
LIFECYCLE_PYBZ_ID=""
LIFECYCLE_GAP_ELIGIBLE=0
LIFECYCLE_GAP_ELIGIBLE_FILE="$COMPARE_EXCHANGE_DIR/.lifecycle-gap-eligible"

lifecycle_gap_reset() {
    LIFECYCLE_GAP_ELIGIBLE=0
    rm -f "$LIFECYCLE_GAP_ELIGIBLE_FILE"
}

lifecycle_gap_allow() {
    LIFECYCLE_GAP_ELIGIBLE=1
    : >"$LIFECYCLE_GAP_ELIGIBLE_FILE"
}

lifecycle_capture_bzr() {
    local name="$1"

    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/${name}.bzr.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.exit"
}

lifecycle_bzr_probe() {
    local name="$1"
    local api="$2"
    local expected_transport="$3"
    local expected_diagnostic="$4"
    # shellcheck disable=SC2034 # The controlled run_bzr fixture reads this dynamic-scope value.
    local LIFECYCLE_BZR_CALL_NAME="$name"
    shift 4

    lifecycle_gap_reset
    RUST_LOG=bzr=debug run_bzr --server-url "$BZ_URL" \
        --server-api-key-env BZR_COMPARE_API_KEY --server-email "$COMPARE_ADMIN_EMAIL" \
        --api "$api" "$@"
    lifecycle_capture_bzr "$name"
    if [[ $BZR_EXIT -ne 0 ]]; then
        if [[ -n $expected_diagnostic && $BZR_EXIT -eq 2 ]] &&
            grep -Fxq "$expected_diagnostic" "$BZR_STDERR"; then
            lifecycle_gap_allow
        fi
        test_fail "bzr $name failed with exit $BZR_EXIT"
        return 1
    fi
    if ! jq -e . "$BZR_STDOUT" >/dev/null; then
        test_fail "bzr $name returned invalid JSON evidence"
        return 1
    fi
    if ! observe_bzr_transport; then
        test_fail "bzr $name transport could not be observed"
        return 1
    fi
    printf '%s\n' "$BZR_TRANSPORT" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.transport"
    lifecycle_gap_allow
    if [[ $BZR_TRANSPORT != "$expected_transport" ]]; then
        test_fail "bzr $name did not use $expected_transport"
        return 1
    fi
    return 0
}

lifecycle_bzr() {
    local name="$1"
    shift
    lifecycle_bzr_probe "$name" rest REST "" "$@"
}

lifecycle_bzr_gap() {
    local name="$1"
    local diagnostic="$2"
    shift 2
    lifecycle_bzr_probe "$name" rest REST "$diagnostic" "$@"
}

lifecycle_bzr_xmlrpc_gap() {
    local name="$1"
    local diagnostic="$2"
    shift 2
    lifecycle_bzr_probe "$name" xmlrpc XMLRPC "$diagnostic" "$@"
}

lifecycle_bzr_no_dispatch() {
    local name="$1"
    # shellcheck disable=SC2034 # The controlled run_bzr fixture reads this dynamic-scope value.
    local LIFECYCLE_BZR_CALL_NAME="$name"
    shift

    lifecycle_gap_reset
    RUST_LOG=bzr=debug run_bzr --server-url "$BZ_URL" \
        --server-api-key-env BZR_COMPARE_API_KEY --server-email "$COMPARE_ADMIN_EMAIL" \
        --api rest "$@"
    lifecycle_capture_bzr "$name"
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "bzr $name failed with exit $BZR_EXIT"
        return 1
    fi
    grep -Eq 'API response|XML-RPC call' "$BZR_STDERR"
    local event_status=$?
    if [[ $event_status -eq 0 ]]; then
        test_fail "bzr $name unexpectedly exercised a client request boundary"
        return 1
    fi
    if [[ $event_status -gt 1 ]]; then
        test_fail "bzr $name boundary evidence could not be read"
        return 1
    fi
    if ! jq -e . "$BZR_STDOUT" >/dev/null; then
        test_fail "bzr $name returned invalid no-dispatch evidence"
        return 1
    fi
    lifecycle_gap_allow
    return 0
}

lifecycle_expect_gap() {
    local issue="$1"

    if [[ $LIFECYCLE_GAP_ELIGIBLE -eq 1 && -f $LIFECYCLE_GAP_ELIGIBLE_FILE ]]; then
        expect_gap "$issue"
    fi
    return 0
}

lifecycle_pybz() {
    local name="$1"
    local operation="$2"
    local payload="$3"
    local input="$COMPARE_EXCHANGE_DIR/${name}.pybz.input.json"
    local output="$COMPARE_EXCHANGE_DIR/${name}.pybz.output.json"

    jq -cn --arg api_key "$BZR_COMPARE_API_KEY" --argjson payload "$payload" \
        '$payload + {api_key:$api_key}' >"$input"
    chmod 600 "$input"
    run_pybz_adapter "$operation" "/work/compare/${input##*/}" \
        "/work/compare/${output##*/}"
    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/${name}.pybz.stdout"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/${name}.pybz.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/${name}.pybz.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.exit"
    if [[ $BZR_EXIT -ne 0 || ! -r $output ]] ||
        ! jq -e '.transport | type == "string" and length > 0' "$output" >/dev/null; then
        test_fail "python-bugzilla $name failed"
        return 1
    fi
    jq -r '.transport' "$output" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.transport"
    jq '.result' "$output" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.result.json"
    return 0
}

lifecycle_positive_id() {
    local path="$1"
    local id

    if ! id=$(jq -er '.id | select(type == "number" and floor == . and . > 0)' "$path"); then
        rm -f "$LIFECYCLE_GAP_ELIGIBLE_FILE"
        return 1
    fi
    printf '%s\n' "$id"
}

lifecycle_ids_are() {
    local source="$1"
    local expected="$2"
    local status

    if ! jq -e 'type == "array" and all(.[]; .id | type == "number")' \
        "$source" >/dev/null; then
        lifecycle_gap_reset
        test_fail "bzr ID evidence had an invalid structure"
        return 1
    fi
    jq -e --argjson expected "$expected" '[.[].id] | sort == $expected' "$source" >/dev/null
    status=$?
    if [[ $status -gt 1 ]]; then
        LIFECYCLE_GAP_ELIGIBLE=0
        test_fail "could not validate bzr ID evidence"
    fi
    return "$status"
}

lifecycle_transport_is() {
    local name="$1"
    local client="$2"
    local expected="$3"
    local actual

    actual=$(<"$COMPARE_EXCHANGE_DIR/${name}.${client}.transport")
    case "$expected:$actual" in
    REST:REST | XMLRPC:XMLRPC) return 0 ;;
    *)
        test_fail "$client $name did not use $expected"
        return 1
        ;;
    esac
}

lifecycle_state() {
    local source="$1"
    local destination="$2"
    local expected_id="${3:-}"

    if [[ -n $expected_id ]] &&
        ! jq -e --argjson expected "$expected_id" '
            (type == "array" and length == 1 and .[0].id == $expected) or
            (type == "object" and .id == $expected)
        ' "$source" >/dev/null; then
        test_fail "result did not identify exactly requested bug $expected_id"
        return 1
    fi

    jq --arg stem "$LIFECYCLE_STEM" --arg bzr "$LIFECYCLE_BZR_SUMMARY" \
        --arg pybz "$LIFECYCLE_PYBZ_SUMMARY" '
        if type == "array" then .[0] else . end |
        .summary |= if . == $bzr or . == $pybz then $stem else . end |
        .component |= if type == "array" then .[0] else . end |
        .version |= if type == "array" then .[0] else . end |
        {product, component, version, summary, op_sys, platform, severity, priority,
         status, resolution, url, whiteboard, cc:(.cc // [] | sort),
         keywords:(.keywords // [] | sort)}
    ' "$source" >"$destination"
}

lifecycle_history() {
    local client="$1"
    local source="$2"
    local destination="$3"
    local expected_id="$4"
    local records

    if [[ $client == bzr ]]; then
        records='[.[] | {field, old_value, new_value}]'
    else
        if ! jq -e --argjson expected "$expected_id" \
            '.bugs | type == "array" and length == 1 and .[0].id == $expected' \
            "$source" >/dev/null; then
            test_fail "history did not return exactly requested bug $expected_id"
            return 1
        fi
        records='[.bugs[0].history[].changes[] |
            {field:.field_name, old_value:.removed, new_value:.added}]'
    fi
    jq --arg stem "$LIFECYCLE_STEM" --arg bzr "$LIFECYCLE_BZR_SUMMARY" \
        --arg pybz "$LIFECYCLE_PYBZ_SUMMARY" "
        $records |
        map(if .field == \"summary\" and (.old_value == \$bzr or .old_value == \$pybz)
            then .old_value = \$stem else . end)
    " "$source" >"$destination"
}

lifecycle_equal() {
    local name="$1"
    local bzr="$2"
    local pybz="$3"

    if ! diff -u "$bzr" "$pybz" >"$COMPARE_EXCHANGE_DIR/${name}.diff"; then
        test_fail "normalized $name differs"
        return 1
    fi
    return 0
}

lifecycle_updated_state_is_persisted() {
    if ! jq -e --arg summary "$LIFECYCLE_UPDATED_SUMMARY" --arg url "$LIFECYCLE_URL" \
        --arg whiteboard "$LIFECYCLE_WHITEBOARD" --arg severity "$LIFECYCLE_UPDATED_SEVERITY" \
        --arg priority "$LIFECYCLE_UPDATED_PRIORITY" '
        .summary == $summary and .url == $url and .whiteboard == $whiteboard and
        .severity == $severity and .priority == $priority
    ' "$1" >/dev/null; then
        test_fail "updated fields were not persisted exactly"
        return 1
    fi
}

lifecycle_update_field() {
    local name="$1" flag="$2" field="$3" value="$4"

    lifecycle_bzr "$name" bug update "$LIFECYCLE_BZR_ID" "$flag" "$value" &&
        lifecycle_pybz "$name" update "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" \
            --arg field "$field" --arg value "$value" \
            '{bug_id:$id,params:{($field):$value}}')"
}

lifecycle_updated_fields_are_in_history() {
    if ! jq -e --arg stem "$LIFECYCLE_STEM" --arg summary "$LIFECYCLE_UPDATED_SUMMARY" \
        --arg url "$LIFECYCLE_URL" --arg whiteboard "$LIFECYCLE_WHITEBOARD" \
        --arg severity "$LIFECYCLE_UPDATED_SEVERITY" --arg priority "$LIFECYCLE_UPDATED_PRIORITY" '
        [{field:"summary",old_value:$stem,new_value:$summary},
         {field:"url",old_value:"",new_value:$url},
         {field:"whiteboard",old_value:"",new_value:$whiteboard},
         {field:"severity",old_value:"normal",new_value:$severity},
         {field:"priority",old_value:"Normal",new_value:$priority}] as $expected |
        [.[] | select(. as $actual | any($expected[]; . == $actual))] == $expected
    ' "$1" >/dev/null; then
        test_fail "exact ordered update transitions were absent from history"
        return 1
    fi
}

test_begin "create" "bug create and first description"
if lifecycle_bzr create bug create --product TestProduct --component TestComponent \
    --summary "$LIFECYCLE_BZR_SUMMARY" --description "$LIFECYCLE_DESCRIPTION" \
    --op-sys Linux --platform PC --priority Normal --severity normal &&
    lifecycle_pybz create create "$(jq -cn --arg summary "$LIFECYCLE_PYBZ_SUMMARY" \
        --arg description "$LIFECYCLE_DESCRIPTION" \
        '{params:{product:"TestProduct",component:"TestComponent",version:"unspecified",
          summary:$summary,description:$description,op_sys:"Linux",platform:"PC",
          priority:"Normal",severity:"normal"}}')"; then
    if LIFECYCLE_BZR_ID=$(lifecycle_positive_id "$COMPARE_EXCHANGE_DIR/create.bzr.stdout.json") &&
        LIFECYCLE_PYBZ_ID=$(lifecycle_positive_id "$COMPARE_EXCHANGE_DIR/create.pybz.result.json"); then
        :
    else
        test_fail "create did not return positive bug IDs"
        LIFECYCLE_BZR_ID=""
        LIFECYCLE_PYBZ_ID=""
    fi
    if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
        lifecycle_bzr create-bzr-comment comment list "$LIFECYCLE_BZR_ID" &&
        cp "$COMPARE_EXCHANGE_DIR/create-bzr-comment.bzr.stdout.json" \
            "$COMPARE_EXCHANGE_DIR/create.bzr.comments.json" &&
        lifecycle_bzr create-pybz-comment comment list "$LIFECYCLE_PYBZ_ID" &&
        cp "$COMPARE_EXCHANGE_DIR/create-pybz-comment.bzr.stdout.json" \
            "$COMPARE_EXCHANGE_DIR/create.pybz.comments.json" &&
        jq -er '.[0].text' "$COMPARE_EXCHANGE_DIR/create.bzr.comments.json" \
            >"$COMPARE_EXCHANGE_DIR/create.bzr.description" &&
        jq -er '.[0].text' "$COMPARE_EXCHANGE_DIR/create.pybz.comments.json" \
            >"$COMPARE_EXCHANGE_DIR/create.pybz.description" &&
        lifecycle_bzr create-bzr-view bug view "$LIFECYCLE_BZR_ID" &&
        lifecycle_bzr create-pybz-view bug view "$LIFECYCLE_PYBZ_ID" &&
        lifecycle_state "$COMPARE_EXCHANGE_DIR/create-bzr-view.bzr.stdout.json" \
            "$COMPARE_EXCHANGE_DIR/create.bzr.normalized.json" "$LIFECYCLE_BZR_ID" &&
        lifecycle_state "$COMPARE_EXCHANGE_DIR/create-pybz-view.bzr.stdout.json" \
            "$COMPARE_EXCHANGE_DIR/create.pybz.normalized.json" "$LIFECYCLE_PYBZ_ID" &&
        lifecycle_equal "create persisted state" "$COMPARE_EXCHANGE_DIR/create.bzr.normalized.json" \
            "$COMPARE_EXCHANGE_DIR/create.pybz.normalized.json" &&
        lifecycle_equal "create first description" "$COMPARE_EXCHANGE_DIR/create.bzr.description" \
            "$COMPARE_EXCHANGE_DIR/create.pybz.description"; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "could not compare created bugs"
    fi
fi

test_begin "query" "bug query"
if lifecycle_bzr query bug list --summary "$LIFECYCLE_BZR_SUMMARY" &&
    lifecycle_pybz query query "$(jq -cn --arg summary "$LIFECYCLE_PYBZ_SUMMARY" \
        '{params:{short_desc:$summary}}')" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/query.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/query.bzr.normalized.json" "$LIFECYCLE_BZR_ID" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/query.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/query.pybz.normalized.json" "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_equal "query persisted state" "$COMPARE_EXCHANGE_DIR/query.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/query.pybz.normalized.json"; then
    test_pass
fi

test_begin "update" "bug update"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_update_field update --summary summary "$LIFECYCLE_UPDATED_SUMMARY" &&
    sleep 1 &&
    lifecycle_update_field update-url --url url "$LIFECYCLE_URL" &&
    sleep 1 &&
    lifecycle_update_field update-whiteboard --whiteboard whiteboard "$LIFECYCLE_WHITEBOARD" &&
    sleep 1 &&
    lifecycle_update_field update-severity --severity severity "$LIFECYCLE_UPDATED_SEVERITY" &&
    sleep 1 &&
    lifecycle_update_field update-priority --priority priority "$LIFECYCLE_UPDATED_PRIORITY" &&
    lifecycle_bzr update-bzr-view bug view "$LIFECYCLE_BZR_ID" &&
    lifecycle_bzr update-pybz-view bug view "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/update-bzr-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/update.bzr.normalized.json" "$LIFECYCLE_BZR_ID" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/update-pybz-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/update.pybz.normalized.json" "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_updated_state_is_persisted "$COMPARE_EXCHANGE_DIR/update.bzr.normalized.json" &&
    lifecycle_updated_state_is_persisted "$COMPARE_EXCHANGE_DIR/update.pybz.normalized.json" &&
    lifecycle_equal "update persisted state" "$COMPARE_EXCHANGE_DIR/update.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/update.pybz.normalized.json"; then
    test_pass
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "create did not produce IDs for update"
fi

test_begin "view" "bug view"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_bzr view-bzr bug view "$LIFECYCLE_BZR_ID" &&
    lifecycle_pybz view view "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" '{bug_id:$id}')" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/view-bzr.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/view.bzr.normalized.json" "$LIFECYCLE_BZR_ID" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/view.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/view.pybz.normalized.json" "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_equal "view persisted state" "$COMPARE_EXCHANGE_DIR/view.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/view.pybz.normalized.json"; then
    test_pass
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "create did not produce IDs for view"
fi

test_begin "history" "bug history"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_bzr history-bzr bug history "$LIFECYCLE_BZR_ID" &&
    lifecycle_pybz history history "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" '{bug_id:$id}')" &&
    lifecycle_history bzr "$COMPARE_EXCHANGE_DIR/history-bzr.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" "$LIFECYCLE_BZR_ID" &&
    lifecycle_history pybz "$COMPARE_EXCHANGE_DIR/history.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json" "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_updated_fields_are_in_history "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" &&
    lifecycle_updated_fields_are_in_history "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json" &&
    lifecycle_equal history "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json"; then
    test_pass
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "create did not produce IDs for history"
fi

LIFECYCLE_SAVED_SEARCH="lifecycle-${BZ_VERSION}-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_FIELD_INITIAL="field-initial-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_FIELD_UPDATED="field-updated-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_COMMENT="tagged-comment-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_COMMENT_TAG="tag-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_BZR_COMMENT="${LIFECYCLE_COMMENT}-bzr"
LIFECYCLE_BZR_COMMENT_TAG="b${LIFECYCLE_COMMENT_TAG}"
LIFECYCLE_WHITEBOARD_EXACT="equals-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_WHITEBOARD_DECOY="${LIFECYCLE_WHITEBOARD_EXACT}-suffix"
LIFECYCLE_BUG_TAG="bug-tag-${LIFECYCLE_RUN_TOKEN}"
LIFECYCLE_BZR_BUG_TAG="${LIFECYCLE_BUG_TAG}-bzr"

test_begin "saved-search" "server saved search"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    seed_server_saved_search "$COMPARE_ADMIN_EMAIL" "$LIFECYCLE_SAVED_SEARCH" \
        "$LIFECYCLE_BZR_ID" "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_pybz saved-search saved_search "$(jq -cn --arg name "$LIFECYCLE_SAVED_SEARCH" \
        '{name:$name}')" &&
    lifecycle_transport_is saved-search pybz XMLRPC &&
    lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/saved-search.pybz.result.json" \
        "[$LIFECYCLE_BZR_ID,$LIFECYCLE_PYBZ_ID]"; then
    if lifecycle_bzr_gap saved-search "error: unexpected argument '--saved-search' found" \
        bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
        lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/saved-search.bzr.stdout.json" \
            "[$LIFECYCLE_BZR_ID,$LIFECYCLE_PYBZ_ID]"; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr saved-search result differed"
    fi
    lifecycle_expect_gap 670
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "saved-search precondition failed"
fi

test_begin "arbitrary-fields" "generic arbitrary fields"
if lifecycle_pybz arbitrary-fields-create generic_fields "$(jq -cn \
    --arg summary "$LIFECYCLE_STEM generic pybz" --arg value "$LIFECYCLE_FIELD_INITIAL" \
    '{action:"create",params:{product:"TestProduct",component:"TestComponent",version:"unspecified",
      summary:$summary,description:"generic fields",op_sys:"Linux",platform:"PC"},
      fields:{whiteboard:$value}}')" &&
    lifecycle_transport_is arbitrary-fields-create pybz XMLRPC &&
    LIFECYCLE_GENERIC_PYBZ_ID=$(lifecycle_positive_id \
        "$COMPARE_EXCHANGE_DIR/arbitrary-fields-create.pybz.result.json") &&
    lifecycle_pybz arbitrary-fields-create-view view "$(jq -cn \
        --argjson id "$LIFECYCLE_GENERIC_PYBZ_ID" '{bug_id:$id}')" &&
    jq -e --arg value "$LIFECYCLE_FIELD_INITIAL" '.whiteboard == $value' \
        "$COMPARE_EXCHANGE_DIR/arbitrary-fields-create-view.pybz.result.json" >/dev/null &&
    lifecycle_pybz arbitrary-fields-update generic_fields "$(jq -cn \
        --argjson id "$LIFECYCLE_GENERIC_PYBZ_ID" --arg value "$LIFECYCLE_FIELD_UPDATED" \
        '{action:"update",bug_id:$id,params:{},fields:{whiteboard:$value}}')" &&
    lifecycle_pybz arbitrary-fields-update-view view "$(jq -cn \
        --argjson id "$LIFECYCLE_GENERIC_PYBZ_ID" '{bug_id:$id}')" &&
    jq -e --arg value "$LIFECYCLE_FIELD_UPDATED" '.whiteboard == $value' \
        "$COMPARE_EXCHANGE_DIR/arbitrary-fields-update-view.pybz.result.json" >/dev/null; then
    if lifecycle_bzr_gap arbitrary-fields-create "error: unexpected argument '--field' found" \
        bug create --product TestProduct --component TestComponent \
        --summary "$LIFECYCLE_STEM generic bzr" --description "generic fields" --op-sys Linux \
        --platform PC --field "whiteboard=$LIFECYCLE_FIELD_INITIAL" &&
        LIFECYCLE_GENERIC_BZR_ID=$(lifecycle_positive_id \
            "$COMPARE_EXCHANGE_DIR/arbitrary-fields-create.bzr.stdout.json") &&
        lifecycle_bzr arbitrary-fields-create-view-bzr bug view "$LIFECYCLE_GENERIC_BZR_ID" &&
        jq -e --arg value "$LIFECYCLE_FIELD_INITIAL" '.whiteboard == $value' \
            "$COMPARE_EXCHANGE_DIR/arbitrary-fields-create-view-bzr.bzr.stdout.json" >/dev/null &&
        lifecycle_bzr_gap arbitrary-fields-update "error: unexpected argument '--field' found" \
            bug update "$LIFECYCLE_GENERIC_BZR_ID" \
            --field "whiteboard=$LIFECYCLE_FIELD_UPDATED" &&
        lifecycle_bzr arbitrary-fields-view bug view "$LIFECYCLE_GENERIC_BZR_ID" &&
        jq -e --arg value "$LIFECYCLE_FIELD_UPDATED" '.whiteboard == $value' \
            "$COMPARE_EXCHANGE_DIR/arbitrary-fields-view.bzr.stdout.json" >/dev/null; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr arbitrary-fields result differed"
    fi
    lifecycle_expect_gap 671
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "arbitrary-fields precondition failed"
fi

test_begin "update-options" "comment tags and minor update"
if [[ -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_pybz update-options update_options "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" \
        --arg comment "$LIFECYCLE_COMMENT" --arg tag "$LIFECYCLE_COMMENT_TAG" \
        '{bug_id:$id,comment:$comment,comment_tags:[$tag],minor_update:true}')" &&
    lifecycle_transport_is update-options pybz REST; then
    if lifecycle_bzr_gap update-options-bzr "error: unexpected argument '--comment-tag' found" \
        bug update "$LIFECYCLE_PYBZ_ID" \
        --comment "$LIFECYCLE_BZR_COMMENT" --comment-tag "$LIFECYCLE_BZR_COMMENT_TAG" \
        --minor-update &&
        lifecycle_bzr update-options-bzr-comment comment list "$LIFECYCLE_PYBZ_ID" &&
        jq -e --arg comment "$LIFECYCLE_BZR_COMMENT" --arg tag "$LIFECYCLE_BZR_COMMENT_TAG" \
            'any(.[]; .text == $comment and (.tags | index($tag)))' \
            "$COMPARE_EXCHANGE_DIR/update-options-bzr-comment.bzr.stdout.json" >/dev/null &&
        lifecycle_bzr_no_dispatch update-options-bzr-request --dry-run \
            bug update "$LIFECYCLE_PYBZ_ID" \
            --comment "$LIFECYCLE_BZR_COMMENT" --comment-tag "$LIFECYCLE_BZR_COMMENT_TAG" \
            --minor-update &&
        jq '.changes' "$COMPARE_EXCHANGE_DIR/update-options-bzr-request.bzr.stdout.json" \
            >"$COMPARE_EXCHANGE_DIR/update-options-bzr.request.json" &&
        jq -e '.minor_update == true' \
            "$COMPARE_EXCHANGE_DIR/update-options-bzr.request.json" >/dev/null; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr update-options result differed"
    fi
    lifecycle_expect_gap 672
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "update-options precondition failed"
fi

test_begin "query-match-types" "whiteboard match types"
if lifecycle_pybz query-match-exact-create generic_fields "$(jq -cn \
    --arg summary "$LIFECYCLE_STEM exact" --arg value "$LIFECYCLE_WHITEBOARD_EXACT" \
    '{action:"create",params:{product:"TestProduct",component:"TestComponent",version:"unspecified",
      summary:$summary,description:"match type",op_sys:"Linux",platform:"PC"},fields:{whiteboard:$value}}')" &&
    LIFECYCLE_MATCH_EXACT_ID=$(lifecycle_positive_id \
        "$COMPARE_EXCHANGE_DIR/query-match-exact-create.pybz.result.json") &&
    lifecycle_pybz query-match-decoy-create generic_fields "$(jq -cn \
    --arg summary "$LIFECYCLE_STEM decoy" --arg value "$LIFECYCLE_WHITEBOARD_DECOY" \
    '{action:"create",params:{product:"TestProduct",component:"TestComponent",version:"unspecified",
      summary:$summary,description:"match type",op_sys:"Linux",platform:"PC"},fields:{whiteboard:$value}}')" &&
    LIFECYCLE_MATCH_DECOY_ID=$(lifecycle_positive_id \
        "$COMPARE_EXCHANGE_DIR/query-match-decoy-create.pybz.result.json") &&
    lifecycle_pybz query-match-substring query "$(jq -cn --arg value "$LIFECYCLE_WHITEBOARD_EXACT" \
        '{params:{status_whiteboard:$value}}')" &&
    lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/query-match-substring.pybz.result.json" \
        "[$LIFECYCLE_MATCH_EXACT_ID,$LIFECYCLE_MATCH_DECOY_ID]" &&
    lifecycle_pybz query-match-equals match_type "$(jq -cn --arg value "$LIFECYCLE_WHITEBOARD_EXACT" \
        '{value:$value,match_type:"equals"}')" &&
    lifecycle_transport_is query-match-equals pybz XMLRPC &&
    lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/query-match-equals.pybz.result.json" \
        "[$LIFECYCLE_MATCH_EXACT_ID]"; then
    if lifecycle_bzr_gap query-match-types \
        "error: unexpected argument '--status-whiteboard-type' found" \
        bug list --whiteboard "$LIFECYCLE_WHITEBOARD_EXACT" \
        --status-whiteboard-type equals &&
        lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/query-match-types.bzr.stdout.json" \
            "[$LIFECYCLE_MATCH_EXACT_ID]"; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr query-match-types result differed"
    fi
    lifecycle_expect_gap 679
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "query-match-types precondition failed"
fi

test_begin "bug-tags" "personal bug tags"
if [[ -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_pybz bug-tags bug_tags "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" \
        --arg tag "$LIFECYCLE_BUG_TAG" '{bug_id:$id,tag:$tag}')" &&
    lifecycle_transport_is bug-tags pybz XMLRPC &&
    jq -e --argjson id "$LIFECYCLE_PYBZ_ID" '[.bugs[].id] | sort == [$id]' \
        "$COMPARE_EXCHANGE_DIR/bug-tags.pybz.result.json" >/dev/null; then
    if lifecycle_bzr_xmlrpc_gap bug-tags-add "error: unrecognized subcommand 'tag'" \
        bug tag "$LIFECYCLE_PYBZ_ID" \
        --add "$LIFECYCLE_BZR_BUG_TAG" &&
        lifecycle_bzr_xmlrpc_gap bug-tags-list "error: unexpected argument '--tag' found" \
            bug list --tag "$LIFECYCLE_BZR_BUG_TAG" &&
        lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/bug-tags-list.bzr.stdout.json" \
            "[$LIFECYCLE_PYBZ_ID]"; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr bug-tags result differed"
    fi
    lifecycle_expect_gap 680
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "bug-tags precondition failed"
fi
