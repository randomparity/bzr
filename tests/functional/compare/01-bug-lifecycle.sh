#!/bin/bash
# Semantic lifecycle comparisons sourced by run-compare.sh.

LIFECYCLE_STEM="bzr-pybz-lifecycle-${BZ_VERSION}-${RANDOM}"
LIFECYCLE_BZR_SUMMARY="$LIFECYCLE_STEM [bzr]"
LIFECYCLE_PYBZ_SUMMARY="$LIFECYCLE_STEM [pybz]"
LIFECYCLE_UPDATED_SUMMARY="$LIFECYCLE_STEM updated"
LIFECYCLE_DESCRIPTION="lifecycle description"
LIFECYCLE_URL="https://example.test/updated"
LIFECYCLE_WHITEBOARD="updated"
LIFECYCLE_BZR_ID=""
LIFECYCLE_PYBZ_ID=""

lifecycle_capture_bzr() {
    local name="$1"

    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/${name}.bzr.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.exit"
    printf 'REST\n' >"$COMPARE_EXCHANGE_DIR/${name}.bzr.transport"
}

lifecycle_bzr() {
    local name="$1"
    shift

    run_bzr --server-url "$BZ_URL" --server-api-key-env BZR_COMPARE_API_KEY \
        --server-email "$COMPARE_ADMIN_EMAIL" --api rest "$@"
    lifecycle_capture_bzr "$name"
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "bzr $name failed with exit $BZR_EXIT"
        return 1
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
    jq -er '.id | select(type == "number" and floor == . and . > 0)' "$path"
}

lifecycle_state() {
    local source="$1"
    local destination="$2"

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
    local records

    if [[ $client == bzr ]]; then
        records='[.[] | {field, old_value, new_value}]'
    else
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

lifecycle_history_equal() {
    local bzr="$1"
    local pybz="$2"
    local bzr_sorted="$COMPARE_EXCHANGE_DIR/history.bzr.compared.json"
    local pybz_sorted="$COMPARE_EXCHANGE_DIR/history.pybz.compared.json"

    jq -S 'sort_by(.field, .old_value, .new_value)' "$bzr" >"$bzr_sorted"
    jq -S 'sort_by(.field, .old_value, .new_value)' "$pybz" >"$pybz_sorted"
    lifecycle_equal history "$bzr_sorted" "$pybz_sorted"
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
            "$COMPARE_EXCHANGE_DIR/create.bzr.normalized.json" &&
        lifecycle_state "$COMPARE_EXCHANGE_DIR/create-pybz-view.bzr.stdout.json" \
            "$COMPARE_EXCHANGE_DIR/create.pybz.normalized.json" &&
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
        "$COMPARE_EXCHANGE_DIR/query.bzr.normalized.json" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/query.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/query.pybz.normalized.json" &&
    lifecycle_equal "query persisted state" "$COMPARE_EXCHANGE_DIR/query.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/query.pybz.normalized.json"; then
    test_pass
fi

test_begin "update" "bug update"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    lifecycle_bzr update bug update "$LIFECYCLE_BZR_ID" --summary "$LIFECYCLE_UPDATED_SUMMARY" \
        --url "$LIFECYCLE_URL" --whiteboard "$LIFECYCLE_WHITEBOARD" &&
    lifecycle_pybz update update "$(jq -cn --argjson id "$LIFECYCLE_PYBZ_ID" \
        --arg summary "$LIFECYCLE_UPDATED_SUMMARY" --arg url "$LIFECYCLE_URL" \
        --arg whiteboard "$LIFECYCLE_WHITEBOARD" \
        '{bug_id:$id,params:{summary:$summary,url:$url,whiteboard:$whiteboard}}')" &&
    lifecycle_bzr update-bzr-view bug view "$LIFECYCLE_BZR_ID" &&
    lifecycle_bzr update-pybz-view bug view "$LIFECYCLE_PYBZ_ID" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/update-bzr-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/update.bzr.normalized.json" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/update-pybz-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/update.pybz.normalized.json" &&
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
        "$COMPARE_EXCHANGE_DIR/view.bzr.normalized.json" &&
    lifecycle_state "$COMPARE_EXCHANGE_DIR/view.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/view.pybz.normalized.json" &&
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
        "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" &&
    lifecycle_history pybz "$COMPARE_EXCHANGE_DIR/history.pybz.result.json" \
        "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json" &&
    lifecycle_history_equal "$COMPARE_EXCHANGE_DIR/history.bzr.normalized.json" \
        "$COMPARE_EXCHANGE_DIR/history.pybz.normalized.json"; then
    test_pass
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "create did not produce IDs for history"
fi
