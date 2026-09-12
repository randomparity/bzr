#!/bin/bash
# Real-server catalogue for RHBZ ExternalBugs and Component.update.

RHBZ_TOKEN=$(unique_name rhbz-ext)
RHBZ_PRODUCT=TestProduct
RHBZ_COMPONENT=TestComponent
RHBZ_EXTERNAL_ID="${RHBZ_TOKEN}-link"
RHBZ_TRACKER="${RHBZ_TOKEN}-tracker"

rhbz_controls_ready() {
    local controls_file="$COMPARE_EXCHANGE_DIR/rhbz-controls.sql"
    local -a controls

    printf '%s\n' \
        "SELECT p.id FROM components c JOIN products p ON p.id = c.product_id JOIN profiles u ON u.userid = c.initialowner WHERE p.name = '$RHBZ_PRODUCT' AND c.name = '$RHBZ_COMPONENT' AND c.isactive = 1 AND u.login_name = '$COMPARE_ADMIN_EMAIL' LIMIT 1;" \
        "SELECT c.id FROM components c JOIN products p ON p.id = c.product_id JOIN profiles u ON u.userid = c.initialowner WHERE p.name = '$RHBZ_PRODUCT' AND c.name = '$RHBZ_COMPONENT' AND c.isactive = 1 AND u.login_name = '$COMPARE_ADMIN_EMAIL' LIMIT 1;" \
        "SELECT COUNT(DISTINCT g.name) FROM profiles p JOIN user_group_map m ON m.user_id = p.userid JOIN groups g ON g.id = m.group_id WHERE p.login_name = '$COMPARE_ADMIN_EMAIL' AND g.name IN ('editbugs', 'editcomponents');" \
        >"$controls_file"
    mapfile -t controls < <(run_bugzilla_sql_file "$controls_file" | awk '/^[0-9]+$/')
    [[ ${controls[0]:-} =~ ^[1-9][0-9]*$ && ${controls[1]:-} =~ ^[1-9][0-9]*$ &&
        ${controls[2]:-} == 2 ]] || return 1
    RHBZ_PRODUCT_ID=${controls[0]}
    RHBZ_COMPONENT_ID=${controls[1]}
}

rhbz_external_state() {
    curl -fsS --get "$BZ_URL/rest/bug/$RHBZ_BUG_ID" \
        --data-urlencode 'include_fields=id,external_bugs' \
        --data-urlencode "Bugzilla_api_key=$BZR_COMPARE_API_KEY" \
        >"$COMPARE_EXCHANGE_DIR/rhbz-external-state.json"
}

rhbz_expect_gap() {
    local command_name="$1" expected_error="$2" expected_usage="$3"
    shift 3
    run_bzr --server "$RESOURCE_SERVER" "$@"
    if [[ $BZR_EXIT -eq 2 ]] && grep -Fxq "$expected_error" "$BZR_STDERR" &&
        grep -Fxq "$expected_usage" "$BZR_STDERR"; then
        test_fail "bzr $command_name surface is not implemented"
        resource_gap_allow
    else
        test_fail "bzr $command_name parser result was not the controlled gap"
    fi
    resource_expect_gap 774
}

test_begin "add" "ExternalBugs add persists a configured tracker link"
resource_gap_reset
if rhbz_controls_ready &&
    printf '%s\n' "INSERT INTO bugs (assigned_to, bug_severity, bug_status, creation_ts, delta_ts, short_desc, op_sys, priority, product_id, rep_platform, reporter, version, component_id, everconfirmed) VALUES (1, 'normal', 'NEW', NOW(), NOW(), '$RHBZ_TOKEN ExternalBugs comparison bug', 'Linux', 'Normal', $RHBZ_PRODUCT_ID, 'PC', 1, 'unspecified', $RHBZ_COMPONENT_ID, 1); SELECT LAST_INSERT_ID();" >"$COMPARE_EXCHANGE_DIR/rhbz-bug.sql" &&
    RHBZ_BUG_ID=$(run_bugzilla_sql_file "$COMPARE_EXCHANGE_DIR/rhbz-bug.sql" | tail -n1) &&
    [[ $RHBZ_BUG_ID =~ ^[1-9][0-9]*$ ]] &&
    printf '%s\n' "INSERT INTO external_bugzilla (url, description, full_url, type) VALUES ('https://tracker.invalid/', '$RHBZ_TRACKER', 'https://tracker.invalid/%%s', 'None'); SELECT id FROM external_bugzilla WHERE description = '$RHBZ_TRACKER';" \
        >"$COMPARE_EXCHANGE_DIR/rhbz-tracker.sql" &&
    RHBZ_TRACKER_ID=$(run_bugzilla_sql_file "$COMPARE_EXCHANGE_DIR/rhbz-tracker.sql" | tail -n1) &&
    [[ $RHBZ_TRACKER_ID =~ ^[1-9][0-9]*$ ]] &&
    resource_pybz rhbz-add externalbugs_add \
        "$(jq -cn --argjson bug_id "$RHBZ_BUG_ID" --argjson tracker_id "$RHBZ_TRACKER_ID" \
            --arg external "$RHBZ_EXTERNAL_ID" '{transport:"XMLRPC",bug_id:$bug_id,tracker_id:$tracker_id,external_bug_id:$external,status:"NEW",description:"created"}')" XMLRPC &&
    rhbz_external_state &&
    jq -e --arg external "$RHBZ_EXTERNAL_ID" --argjson tracker "$RHBZ_TRACKER_ID" \
        '.bugs[0].external_bugs | any(.[]; .ext_bz_bug_id == $external and .type.id == $tracker)' \
        "$COMPARE_EXCHANGE_DIR/rhbz-external-state.json" >/dev/null; then
    rhbz_expect_gap add "error: unrecognized subcommand 'external-bug'" \
        'Usage: bzr bug [OPTIONS] <COMMAND>' bug external-bug add "$RHBZ_BUG_ID"
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail 'ExternalBugs add positive control failed'
fi

test_begin "update" "ExternalBugs update persists changed fields"
resource_gap_reset
if [[ -n ${RHBZ_BUG_ID:-} && -n ${RHBZ_TRACKER_ID:-} ]] &&
    resource_pybz rhbz-update externalbugs_update \
    "$(jq -cn --argjson bug_id "$RHBZ_BUG_ID" --argjson tracker_id "$RHBZ_TRACKER_ID" \
        --arg external "$RHBZ_EXTERNAL_ID" '{transport:"XMLRPC",bug_id:$bug_id,tracker_id:$tracker_id,external_bug_id:$external,status:"ASSIGNED",description:"updated"}')" XMLRPC &&
    rhbz_external_state &&
    jq -e --arg external "$RHBZ_EXTERNAL_ID" \
        '.bugs[0].external_bugs | any(.[]; .ext_bz_bug_id == $external and .ext_status == "ASSIGNED" and .ext_description == "updated")' \
        "$COMPARE_EXCHANGE_DIR/rhbz-external-state.json" >/dev/null; then
    rhbz_expect_gap update "error: unrecognized subcommand 'external-bug'" \
        'Usage: bzr bug [OPTIONS] <COMMAND>' bug external-bug update "$RHBZ_BUG_ID"
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail 'ExternalBugs update positive control failed'
fi

test_begin "remove" "ExternalBugs remove deletes the persisted link"
resource_gap_reset
if [[ -n ${RHBZ_BUG_ID:-} && -n ${RHBZ_TRACKER_ID:-} ]] &&
    resource_pybz rhbz-remove externalbugs_remove \
    "$(jq -cn --argjson bug_id "$RHBZ_BUG_ID" --argjson tracker_id "$RHBZ_TRACKER_ID" \
        --arg external "$RHBZ_EXTERNAL_ID" '{transport:"XMLRPC",bug_id:$bug_id,tracker_id:$tracker_id,external_bug_id:$external}')" XMLRPC &&
    rhbz_external_state &&
    jq -e --arg external "$RHBZ_EXTERNAL_ID" \
        '.bugs[0].external_bugs | all(.[]; .ext_bz_bug_id != $external)' \
        "$COMPARE_EXCHANGE_DIR/rhbz-external-state.json" >/dev/null; then
    rhbz_expect_gap remove "error: unrecognized subcommand 'external-bug'" \
        'Usage: bzr bug [OPTIONS] <COMMAND>' bug external-bug remove "$RHBZ_BUG_ID"
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail 'ExternalBugs remove positive control failed'
fi

test_begin "component-update" "Component.update persists the changed component"
resource_gap_reset
if resource_pybz rhbz-component-update component_update \
    "$(jq -cn --arg product "$RHBZ_PRODUCT" --arg component "$RHBZ_COMPONENT" --arg owner "$COMPARE_ADMIN_EMAIL" \
        '{transport:"XMLRPC",params:{product:$product,component:$component,initialowner:$owner,description:"updated RHBZ component",is_active:false}}')" XMLRPC &&
    curl -fsS --get "$BZ_URL/rest/product" \
        --data-urlencode "names=$RHBZ_PRODUCT" \
        --data-urlencode "Bugzilla_api_key=$BZR_COMPARE_API_KEY" >"$COMPARE_EXCHANGE_DIR/rhbz-component-view.json" &&
    jq -e --arg component "$RHBZ_COMPONENT" \
        '.products[0].components[] | select(.name == $component) | .description == "updated RHBZ component" and .is_active == false' \
        "$COMPARE_EXCHANGE_DIR/rhbz-component-view.json" >/dev/null; then
    rhbz_expect_gap component-update "error: unrecognized subcommand 'update'" \
        'Usage: bzr component [OPTIONS] <COMMAND>' component update
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail 'Component.update positive control failed'
fi
